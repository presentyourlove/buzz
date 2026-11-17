#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Honey2 每日進化建議書 - Direct Message (DM) 私訊直達發送器

功能：
1. 自動從 managed-agents.json 載入 Honey2 之憑證與認證標籤
2. 匯入 DailyEvolution 核心分析引擎，解析當日討論與工作紀錄
3. 自動產出結構化之系統進化提案並存檔於 C:\\Users\\user\\.buzz\\PLANS\\
4. 透過 Buzz CLI 直接發送至主理人專屬 Direct Message 通道 (af946a63-dd43-4050-baea-65140d47fc42)
5. 附帶快速審核批示指引，方便主理人在手機或電腦端單擊/回覆批示
"""

import os
import sys
import json
import re
import argparse
import subprocess
from datetime import datetime
from pathlib import Path

# 加入 daily_evolution 模組路徑
SCRIPT_DIR = Path(__file__).resolve().parent
EVOLUTION_MODULE_DIR = Path(r"C:\Users\user\.buzz\PLANS\daily_evolution")
if str(EVOLUTION_MODULE_DIR) not in sys.path:
    sys.path.insert(0, str(EVOLUTION_MODULE_DIR))

try:
    from daily_evolution import DailyEvolution
except ImportError as e:
    print(f"[錯誤] 無法匯入 DailyEvolution 模組: {e}")
    sys.exit(1)

MANAGED_AGENTS_PATH = r"C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\managed-agents.json"
BUZZ_CLI_PATH = r"C:\Users\user\AppData\Local\Buzz\buzz.exe"
DEFAULT_RELAY_URL = "wss://presentyourlove.communities.buzz.xyz"
USER_DM_CHANNEL = "af946a63-dd43-4050-baea-65140d47fc42"
ENGINEERING_CHANNEL = "5ca019f6-1a27-592a-b015-4baf0eb33572"
WORKSPACE_ROOT = r"C:\Users\user\.buzz"


def sanitize_mentions(text: str) -> str:
    """將未被代碼反引號包裹的 @提及 轉為行內代碼，防止 Buzz CLI 進行成員比對失敗拋錯"""
    return re.sub(r'(?<!`)@([a-zA-Z0-9_\u4e00-\u9fa5]+)(?!`)', r'`@\1`', text)


def load_honey2_credentials():
    """從 managed-agents.json 取得 Honey2 的 nsec 私鑰、auth_tag 與 pubkey"""
    if not os.path.exists(MANAGED_AGENTS_PATH):
        print(f"[錯誤] 找不到 managed-agents.json: {MANAGED_AGENTS_PATH}")
        return None, None, None

    try:
        with open(MANAGED_AGENTS_PATH, "r", encoding="utf-8") as f:
            agents = json.load(f)
        for agent in agents:
            if (
                agent.get("name") == "Honey2"
                and agent.get("is_active") is True
                and agent.get("private_key_nsec")
            ):
                nsec = agent.get("private_key_nsec")
                auth_tag = agent.get("auth_tag")
                pubkey = agent.get("pubkey", "")
                if isinstance(auth_tag, list):
                    auth_tag = json.dumps(auth_tag)
                return nsec, auth_tag, pubkey
    except Exception as e:
        print(f"[錯誤] 解析 managed-agents.json 失敗: {e}")
    return None, None, None


def send_dm_message(nsec: str, auth_tag: str, pubkey: str, content: str) -> bool:
    """調用 Buzz CLI 將訊息推送至主理人專屬 DM 通道"""
    env = os.environ.copy()
    env["BUZZ_RELAY_URL"] = DEFAULT_RELAY_URL
    env["BUZZ_PRIVATE_KEY"] = nsec
    if auth_tag:
        env["BUZZ_AUTH_TAG"] = auth_tag
    env["PYTHONIOENCODING"] = "utf-8"

    sanitized_content = sanitize_mentions(content)

    cmd = [
        BUZZ_CLI_PATH,
        "messages", "send",
        "--channel", USER_DM_CHANNEL,
        "--content", "-"
    ]
    if pubkey:
        cmd.extend(["--mention", pubkey])

    print(f"📤 正在發送每日進化建議書至主理人專屬私訊 ({USER_DM_CHANNEL})...")
    try:
        flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
        res = subprocess.run(
            cmd,
            input=sanitized_content,
            capture_output=True,
            text=True,
            env=env,
            encoding="utf-8",
            timeout=30,
            creationflags=flags
        )
        if res.returncode == 0:
            print("   ✅ [成功] 每日進化建議書已送達主理人私訊視窗！")
            return True
        else:
            print(f"   ⚠️ [失敗] Buzz CLI 發送回傳代碼非 0: {res.stderr}")
            return False
    except Exception as e:
        print(f"   ❌ [錯誤] 發送私訊失敗: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Honey2 每日進化建議書私訊推送")
    parser.add_argument("--dry-run", action="store_true", help="演練模式 (不發送私訊)")
    parser.add_argument("--also-post-channel", action="store_true", help="同步發布至公共工程頻道")
    args = parser.parse_args()

    print("==========================================================")
    print("🐝 Honey2 每日進化建議書 - Direct Message 私訊排程啟動中")
    print("==========================================================")

    nsec, auth_tag, pubkey = load_honey2_credentials()
    if not nsec:
        print("[嚴重錯誤] 無法取得 Honey2 憑證，終止執行。")
        sys.exit(1)

    print("🔍 正在初始化每日進化核心引擎...")
    evolution = DailyEvolution(
        channel_id=ENGINEERING_CHANNEL,
        relay_url=DEFAULT_RELAY_URL,
        private_key=nsec,
        auth_tag=auth_tag,
        buzz_cli_path=BUZZ_CLI_PATH,
        workspace_root=WORKSPACE_ROOT,
    )

    # 執行分析流程（不直接由內部發送頻道，由本腳本接管格式與私訊發送）
    result = evolution.run(post_to_channel=args.also_post_channel)
    proposal_path = result.get("proposal_path", "")
    channel_proposal = result.get("channel_proposal", "")

    # channel_proposal 已經是極簡雙層速讀卡片，直接作為專屬私訊發送
    dm_content = channel_proposal

    if args.dry_run:
        print("💡 [演練模式] 私訊預覽內容如下：")
        print(dm_content)
        return

    # 發送至主理人專屬 DM
    send_dm_message(nsec, auth_tag, pubkey, dm_content)


if __name__ == "__main__":
    main()
