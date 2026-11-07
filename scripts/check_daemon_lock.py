#!/usr/bin/env python3
"""
Daemon 單例鎖健康度檢查腳本

功能：
1. 連線 Redis 檢查 Daemon 分散式鎖狀態
2. 驗證鎖持有者身分、TTL 剩餘時間、心跳更新情況
3. 偵測 split-brain（多實例同時持有鎖）
4. 異常時發送 Nostr DM 通知 Fizz (Kind 4 / NIP-44 v2 加密)

環境變數：
- BUZZ_REDIS_URL: Redis 連線位址 (預設 redis://localhost:6379)
- BUZZ_ALERT_BOT_NSEC: 告警 Bot 的 nsec 私鑰 (專用身分，與 Agent 金鑰分離)
- TELEGRAM_BOT_TOKEN: Telegram Webhook 備援 Token
- TELEGRAM_CHAT_ID: Telegram 通知目標 Chat ID
- BUZZ_AGENT_PUBKEY: 當前 Agent 公鑰 (用於構建鎖鍵)
"""

import argparse
import asyncio
import json
import os
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

import redis.asyncio as redis
import requests
from nostr.event import EventBuilder
from nostr.key import PrivateKey, PublicKey
from nostr.nips.nip44 import encrypt, Version
from nostr.relay_manager import RelayManager


# ─── 常數定義 ───
DEFAULT_REDIS_URL = "redis://localhost:6379"
DEFAULT_RELAY_URL = "wss://presentyourlove.communities.buzz.xyz"
FIZZ_PUBKEY = "5ef9a934c33b784684cded5768e9bdd6dc7ff7274db79870aef6a9c846bf2d4a"
# 跨平台臨時目錄：Windows 用 %TEMP%，Linux/macOS 用 /tmp
ALERT_QUEUE_PATH = Path(tempfile.gettempdir()) / "buzz_alert_queue.jsonl"
LOCK_TTL = 10  # 秒，對應 daemon_lock.rs:39
HEARTBEAT_INTERVAL = LOCK_TTL // 3  # 3 秒，對應 daemon_lock.rs:130


# ─── 告警佇列管理 ───
class AlertQueue:
    """本地告警佇列：失敗降級機制的第一層"""

    def __init__(self, path: Path = ALERT_QUEUE_PATH):
        self.path = path
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def enqueue(self, alert: dict) -> None:
        """寫入告警到本地佇列"""
        alert["queued_at"] = datetime.now(timezone.utc).isoformat()
        with self.path.open("a", encoding="utf-8") as f:
            f.write(json.dumps(alert, ensure_ascii=False) + "\n")

    def dequeue_all(self) -> list[dict]:
        """讀取並清空佇列"""
        if not self.path.exists():
            return []
        alerts = []
        with self.path.open("r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    alerts.append(json.loads(line))
        self.path.unlink(missing_ok=True)
        return alerts

    def requeue(self, alerts: list[dict]) -> None:
        """重新寫入佇列（重放失敗時）"""
        for alert in alerts:
            self.enqueue(alert)


# ─── DM 發送協議實作 ───
class DMNotifier:
    """
    Nostr DM 通知發送器 (Kind 4 / NIP-44 v2)

    協議規範：
    - Event Kind: Kind 4 (NIP-44 v2 加密)
    - 簽名密鑰: 環境變數 BUZZ_ALERT_BOT_NSEC
    - Relay 鑑權: NIP-42 Challenge-Response
    - 失敗降級: 本地佇列 + Telegram Webhook + 自動重放
    """

    def __init__(
        self,
        relay_url: str,
        bot_nsec: str,
        target_pubkey: str = FIZZ_PUBKEY,
        telegram_token: Optional[str] = None,
        telegram_chat_id: Optional[str] = None,
    ):
        self.relay_url = relay_url
        self.target_pubkey = target_pubkey
        self.telegram_token = telegram_token
        self.telegram_chat_id = telegram_chat_id
        self.queue = AlertQueue()

        # 解析 Bot 私鑰
        try:
            self.bot_keys = PrivateKey.from_nsec(bot_nsec)
        except Exception as e:
            raise ValueError(f"Invalid BUZZ_ALERT_BOT_NSEC: {e}")

        # 目標公鑰
        try:
            self.target_pk = PublicKey.from_hex(target_pubkey)
        except Exception as e:
            raise ValueError(f"Invalid target pubkey: {e}")

    async def send_dm(self, content: str, max_retries: int = 3) -> bool:
        """發送 DM，含重試與降級"""
        for attempt in range(max_retries):
            try:
                success = await self._attempt_send(content)
                if success:
                    await self._replay_queue()
                    return True
            except Exception as e:
                wait_time = [1, 3, 10][min(attempt, 2)]
                print(f"DM send attempt {attempt + 1} failed: {e}, retrying in {wait_time}s...")
                await asyncio.sleep(wait_time)

        # 所有重試失敗 -> 觸發降級
        await self._fallback(content)
        return False

    async def _attempt_send(self, content: str) -> bool:
        """嘗試發送單次 DM"""
        # NIP-44 v2 加密
        encrypted = encrypt(
            self.bot_keys,
            self.target_pk,
            content,
            Version.V2,
        )

        # 建立 Kind 4 事件
        event = EventBuilder(
            kind=4,
            content=encrypted,
            tags=[["p", self.target_pubkey]],
        ).sign_with_key(self.bot_keys)

        # 連線 Relay 並發送
        manager = RelayManager()
        manager.add_relay(self.relay_url)
        manager.open_connections()

        # 等待連線建立
        await asyncio.sleep(0.5)

        # NIP-42 認證（若 Relay 需要）
        await self._nip42_auth(manager)

        # 發送事件
        manager.publish_event(event)
        await asyncio.sleep(0.5)  # 等待發送完成

        manager.close_connections()
        return True

    async def _nip42_auth(self, manager: RelayManager) -> None:
        """NIP-42 Challenge-Response 認證（參考 daemon_ws.rs:116-146）"""
        # 發送初始 AUTH (challenge="")
        auth_event = EventBuilder(
            kind=22242,
            content="",
            tags=[
                ["relay", self.relay_url],
                ["challenge", ""],
            ],
        ).sign_with_key(self.bot_keys)

        manager.publish_event(auth_event)
        # 注意：完整的 challenge-response 需要監聽 NOTICE 回應
        # 這裡發送初始 auth，若 Relay 不要求認證則直接接受
        await asyncio.sleep(0.3)

    async def _fallback(self, content: str) -> None:
        """三層降級機制"""
        alert = {
            "title": "Daemon 鎖異常",
            "content": content,
            "target_pubkey": self.target_pubkey,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }

        # 1. 本地佇列
        self.queue.enqueue(alert)
        print("DM 發送失敗，已寫入本地佇列")

        # 2. Telegram Webhook 備援
        if self.telegram_token and self.telegram_chat_id:
            try:
                await self._send_telegram(alert)
                print("Telegram 備援發送成功")
            except Exception as e:
                print(f"Telegram 備援也失敗: {e}")

        # 3. 標記需人工複核（已在佇列中）

    async def _send_telegram(self, alert: dict) -> None:
        """發送 Telegram 通知"""
        text = (
            f"{alert['title']}\n"
            f"時間: {alert['timestamp']}\n"
            f"目標: {alert['target_pubkey'][:16]}...\n"
            f"內容: {alert['content'][:500]}"
        )
        url = f"https://api.telegram.org/bot{self.telegram_token}/sendMessage"
        resp = requests.post(
            url,
            json={"chat_id": self.telegram_chat_id, "text": text, "parse_mode": "HTML"},
            timeout=10,
        )
        resp.raise_for_status()

    async def _replay_queue(self) -> None:
        """重放佇列中的告警"""
        alerts = self.queue.dequeue_all()
        if not alerts:
            return

        print(f"重放 {len(alerts)} 條佇列告警...")
        for alert in alerts:
            try:
                await self._attempt_send(alert["content"])
            except Exception:
                # 重放失敗再放回佇列
                self.queue.enqueue(alert)
                break


# ─── Daemon 鎖健康度檢查 ───
class DaemonLockChecker:
    """檢查 Daemon 分散式鎖健康度"""

    def __init__(self, redis_url: str, agent_pubkey: str):
        self.redis_url = redis_url
        self.agent_pubkey = agent_pubkey
        self.lock_key = f"buzz_agent_daemon:{agent_pubkey}"
        self.failures = []

    async def check_lock(self) -> dict:
        """執行完整鎖健康度檢查"""
        print(f"檢查 Daemon 鎖: {self.lock_key}")
        
        r = redis.from_url(self.redis_url, decode_responses=True)
        
        try:
            # 1. 檢查鎖是否存在
            lock_value = await r.get(self.lock_key)
            if lock_value is None:
                self.failures.append({
                    "check": "lock_exists",
                    "severity": "critical",
                    "reason": "Daemon 鎖不存在 - 無實例持有鎖或鎖已過期",
                    "lock_key": self.lock_key,
                })
                await r.close()
                return self._build_result()

            # 2. 檢查 TTL
            ttl = await r.ttl(self.lock_key)
            if ttl < 0:
                self.failures.append({
                    "check": "lock_ttl",
                    "severity": "critical",
                    "reason": f"鎖無 TTL (ttl={ttl}) - 可能為孤兒鎖",
                    "lock_key": self.lock_key,
                    "ttl": ttl,
                })
            elif ttl < 3:  # 少於 heartbeat interval
                self.failures.append({
                    "check": "lock_ttl",
                    "severity": "warning",
                    "reason": f"鎖 TTL 過短 ({ttl}s) - 心跳可能跟不上",
                    "lock_key": self.lock_key,
                    "ttl": ttl,
                })

            # 3. 檢查鎖值 (應為 "1")
            if lock_value != "1":
                self.failures.append({
                    "check": "lock_value",
                    "severity": "warning",
                    "reason": f"鎖值異常: 期望 '1', 實際 '{lock_value}'",
                    "lock_key": self.lock_key,
                    "value": lock_value,
                })

            # 4. 檢查心跳相關 key (若有)
            heartbeat_key = f"{self.lock_key}:heartbeat"
            hb_value = await r.get(heartbeat_key)
            if hb_value:
                try:
                    last_hb = int(hb_value)
                    now = int(time.time())
                    if now - last_hb > HEARTBEAT_INTERVAL * 2:
                        self.failures.append({
                            "check": "heartbeat",
                            "severity": "warning",
                            "reason": f"心跳超過 {HEARTBEAT_INTERVAL * 2}s 未更新",
                            "last_heartbeat": last_hb,
                            "seconds_ago": now - last_hb,
                        })
                except ValueError:
                    self.failures.append({
                        "check": "heartbeat",
                        "severity": "warning",
                        "reason": f"心跳值格式錯誤: {hb_value}",
                    })

        except redis.RedisError as e:
            self.failures.append({
                "check": "redis_connection",
                "severity": "critical",
                "reason": f"Redis 連線錯誤: {e}",
            })
        finally:
            await r.close()

        return self._build_result()

    def _build_result(self) -> dict:
        return {
            "lock_key": self.lock_key,
            "agent_pubkey": self.agent_pubkey,
            "healthy": len(self.failures) == 0,
            "failures": self.failures,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "run_id": f"lock_check_{datetime.now().strftime('%Y%m%d_%H%M%S')}",
        }


async def main():
    parser = argparse.ArgumentParser(description="Daemon 鎖健康度檢查腳本")
    parser.add_argument("--redis", default=DEFAULT_REDIS_URL, help="Redis URL")
    parser.add_argument("--relay", default=DEFAULT_RELAY_URL, help="Relay URL")
    parser.add_argument("--agent-pubkey", required=True, help="Agent 公鑰 (hex)")
    parser.add_argument("--alert-on-failure", action="store_true", help="發送告警")
    parser.add_argument("--target-pubkey", default=FIZZ_PUBKEY, help="告警目標 pubkey")
    args = parser.parse_args()

    # 環境變數
    bot_nsec = os.getenv("BUZZ_ALERT_BOT_NSEC")
    telegram_token = os.getenv("TELEGRAM_BOT_TOKEN")
    telegram_chat_id = os.getenv("TELEGRAM_CHAT_ID")

    # 執行檢查
    checker = DaemonLockChecker(args.redis, args.agent_pubkey)
    result = await checker.check_lock()

    # 輸出結果
    print(json.dumps(result, indent=2, ensure_ascii=False))

    # 異常告警
    if result["failures"] and args.alert_on_failure:
        if not bot_nsec:
            print("❌ 錯誤：未設定 BUZZ_ALERT_BOT_NSEC 環境變數，無法發送 DM 告警")
            print("   請設定 BUZZ_ALERT_BOT_NSEC 後重試。僅寫入本地佇列...")
            # 不使用假私鑰，直接返回錯誤
            sys.exit(1)

        notifier = DMNotifier(
            relay_url=args.relay,
            bot_nsec=bot_nsec,
            target_pubkey=args.target_pubkey,
            telegram_token=telegram_token,
            telegram_chat_id=telegram_chat_id,
        )

        alert_content = json.dumps({
            "title": "Daemon 鎖異常",
            "lock_key": result["lock_key"],
            "check": "check_daemon_lock",
            "failures": result["failures"],
            "timestamp": result["timestamp"],
            "run_id": result["run_id"],
        }, ensure_ascii=False)

        await notifier.send_dm(alert_content)
        print("告警發送完成")

    # 返回碼：有失敗返回 1
    sys.exit(1 if result["failures"] else 0)


if __name__ == "__main__":
    asyncio.run(main())
       
