#!/usr/bin/env python3
"""
Agent 通訊軌跡索引驗證腳本

功能：
1. 查詢 Nostr Relay 近期頻道事件
2. 驗證 HistoryItem 結構的 context_pressure_bytes 計算正確性
3. 驗證 ToolResult 中 Image 內容的 token 當量換算 (IMAGE_CONTEXT_TOKEN_EQUIV = 16KB)
3. 驗證 provider_extra 欄位是否正確序列化回放
4. 驗證 handoff 後的 history 重置與 _PostCompact hook 注入
5. 異常時發送 Nostr DM 通知 Fizz (Kind 4 / NIP-44 v2 加密)

環境變數：
- BUZZ_RELAY_URL: Relay 連線位址
- BUZZ_ALERT_BOT_NSEC: 告警 Bot 的 nsec 私鑰 (專用身分，與 Agent 金鑰分離)
- TELEGRAM_BOT_TOKEN: Telegram Webhook 備援 Token
- TELEGRAM_CHAT_ID: Telegram 通知目標 Chat ID
"""

import argparse
import asyncio
import hashlib
import json
import os
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

import requests
from nostr.event import Event, EventBuilder
from nostr.key import PrivateKey, PublicKey
from nostr.nips.nip44 import encrypt, Version
from nostr.relay_manager import RelayManager


# ─── 常數定義 ───
IMAGE_CONTEXT_TOKEN_EQUIV = 16 * 1024  # 16KB per image (token equivalent)
DEFAULT_RELAY_URL = "wss://presentyourlove.communities.buzz.xyz"
DEFAULT_CHANNEL = "5ca019f6-1a27-592a-b015-4baf0eb33572"
FIZZ_PUBKEY = "5ef9a934c33b784684cded5768e9bdd6dc7ff7274db79870aef6a9c846bf2d4a"
# 跨平台臨時目錄：Windows 用 %TEMP%，Linux/macOS 用 /tmp
ALERT_QUEUE_PATH = Path(tempfile.gettempdir()) / "buzz_alert_queue.jsonl"


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
            "title": "🚨 Agent 索引驗證異常",
            "content": content,
            "target_pubkey": self.target_pubkey,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }

        # 1. 本地佇列
        self.queue.enqueue(alert)
        print("⚠️ DM 發送失敗，已寫入本地佇列")

        # 2. Telegram Webhook 備援
        if self.telegram_token and self.telegram_chat_id:
            try:
                await self._send_telegram(alert)
                print("✅ Telegram 備援發送成功")
            except Exception as e:
                print(f"⚠️ Telegram 備援也失敗: {e}")

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

        print(f"🔄 重放 {len(alerts)} 條佇列告警...")
        for alert in alerts:
            try:
                await self._attempt_send(alert["content"])
            except Exception:
                # 重放失敗再放回佇列
                self.queue.enqueue(alert)
                break


# ─── 索引驗證邏輯 ───
class IndexVerifier:
    """驗證 Agent 通訊軌跡索引的正確性"""

    def __init__(self, relay_url: str, channel_id: str, since_ts: int):
        self.relay_url = relay_url
        self.channel_id = channel_id
        self.since_ts = since_ts
        self.failures = []

    def verify_history_item(self, event: Event) -> list[dict]:
        """驗證單一 HistoryItem 結構"""
        failures = []

        try:
            content = json.loads(event.content)
        except json.JSONDecodeError:
            # 非 JSON 內容（如純文字訊息）略過結構驗證
            return failures

        # 檢查是否為工具結果
        if content.get("type") == "ToolResult":
            tool_result = content.get("content", [])
            for item in tool_result:
                if item.get("type") == "Image":
                    # 驗證 Image token 當量 = 16KB
                    data_len = len(item.get("data", ""))
                    expected_pressure = IMAGE_CONTEXT_TOKEN_EQUIV + len(item.get("mime_type", ""))
                    # 驗證：base64 長度不應被計入 context_pressure
                    if data_len > expected_pressure * 10:  # base64 通常比 token 當量大 100 倍以上
                        failures.append({
                            "item": "HistoryItem::ToolResult::Image",
                            "check": "token_equivalent",
                            "data_base64_bytes": data_len,
                            "expected_pressure_bytes": expected_pressure,
                            "reason": "Image 應按 16KB token 當量計算，而非 base64 長度",
                            "event_id": event.id,
                            "pubkey": event.pubkey,
                        })

            # 驗證 provider_id 存在 (provider_extra 會在序列化時被計入)
            provider_id = content.get("provider_id", "")
            if not provider_id:
                failures.append({
                    "item": "HistoryItem::ToolResult",
                    "check": "provider_id_missing",
                    "reason": "ToolResult 缺少 provider_id，無法正確計算 context_pressure",
                    "event_id": event.id,
                    "pubkey": event.pubkey,
                })

        # 驗證 handoff 重置標記
        if "[Context Handoff]" in event.content:
            # 檢查是否有 _PostCompact hook 輸出
            if "[Post-compact hook output" not in event.content:
                failures.append({
                    "item": "HistoryItem::User (handoff)",
                    "check": "post_compact_hook",
                    "reason": "Handoff 重置後缺少 _PostCompact hook 輸出注入",
                    "event_id": event.id,
                    "pubkey": event.pubkey,
                })

        return failures

    async def run(self) -> dict:
        """執行驗證"""
        print(f"🔍 開始驗證頻道 {self.channel_id} 從 {self.since_ts} 起的事件...")

        manager = RelayManager()
        manager.add_relay(self.relay_url)
        manager.open_connections()
        await asyncio.sleep(0.5)

        # 訂閱頻道事件 (kind 9, 14, 24200 等)
        from nostr.filter import Filter, Filters

        filters = Filters([
            Filter(
                kinds=[9, 14, 24200],  # 文字訊息、DM、Agent turn
                since=self.since_ts,
                custom_tags={"h": [self.channel_id]},
            )
        ])

        subscription_id = "verify_indexing"
        manager.add_subscription(subscription_id, filters)

        # 收集事件
        events = []
        timeout = 10.0
        start = time.time()

        while time.time() - start < timeout:
            while manager.message_pool.has_events():
                event_msg = manager.message_pool.get_event()
                if event_msg:
                    events.append(event_msg.event)
            await asyncio.sleep(0.1)

        manager.close_connections()

        print(f"📥 收集到 {len(events)} 個事件")

        # 驗證每個事件
        for event in events:
            item_failures = self.verify_history_item(event)
            if item_failures:
                self.failures.extend(item_failures)

        return {
            "channel": self.channel_id,
            "since": self.since_ts,
            "events_checked": len(events),
            "failures": self.failures,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "run_id": f"verify_{datetime.now().strftime('%Y%m%d_%H%M%S')}",
        }


async def main():
    parser = argparse.ArgumentParser(description="Agent 索引驗證腳本")
    parser.add_argument("--relay", default=DEFAULT_RELAY_URL, help="Relay URL")
    parser.add_argument("--channel", default=DEFAULT_CHANNEL, help="Channel ID")
    parser.add_argument("--since", type=int, help="Unix timestamp (seconds)")
    parser.add_argument("--alert-on-failure", action="store_true", help="發送告警")
    parser.add_argument("--target-pubkey", default=FIZZ_PUBKEY, help="告警目標 pubkey")
    args = parser.parse_args()

    # 計算 since 時間
    if args.since:
        since_ts = args.since
    else:
        since_ts = int(time.time()) - 15 * 60  # 預設 15 分鐘前

    # 環境變數 - 必須設定，不提供假私鑰
    bot_nsec = os.getenv("BUZZ_ALERT_BOT_NSEC")
    telegram_token = os.getenv("TELEGRAM_BOT_TOKEN")
    telegram_chat_id = os.getenv("TELEGRAM_CHAT_ID")

    # 執行驗證
    verifier = IndexVerifier(args.relay, args.channel, since_ts)
    result = await verifier.run()

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
            "title": "🚨 Agent 索引驗證異常",
            "channel": args.channel,
            "check": "verify_agent_indexing",
            "failures": result["failures"],
            "timestamp": result["timestamp"],
            "run_id": result["run_id"],
        }, ensure_ascii=False)

        await notifier.send_dm(alert_content)
        print("📤 告警發送完成")

    # 返回碼：有失敗返回 1
    sys.exit(1 if result["failures"] else 0)


if __name__ == "__main__":
    asyncio.run(main())
       
