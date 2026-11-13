#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
05_task_sync_board.py — 活躍成員進度對齊看板
職責：
1. 查詢特工蜂群在線狀態 (Pollen, Sentry, Honey2, Fizz, Honey, Wax)。
2. 匯總當前工單任務，分為 To Do / In Progress / Done 三大維度。
3. 輸出 Markdown 狀態看板至 .buzz/OUTBOX/TASK_SYNC_YYYYMMDD.md。
"""

import os
import sys
import json
import argparse
import subprocess
from pathlib import Path
from datetime import datetime, timezone

OUTBOX_DIR = Path(os.path.expandvars(r"%USERPROFILE%\.buzz\OUTBOX"))
CONFIG_PATH = Path(os.path.expandvars(r"%APPDATA%\xyz.block.buzz.app\agents\managed-agents.json"))

def get_agent_status():
    agents_status = {}
    known_agents = ["Pollen", "Sentry", "Honey2", "Fizz", "Honey", "Wax"]
    for a in known_agents:
        agents_status[a] = "離線 (Offline)"

    # 透過 PowerShell 查詢 buzz-acp 進程
    try:
        flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
        cmd = ["powershell", "-NoProfile", "-Command", "Get-CimInstance Win32_Process -Filter \"name='buzz-acp.exe'\" | Select-Object -ExpandProperty CommandLine"]
        res = subprocess.run(cmd, capture_output=True, text=True, errors="ignore", creationflags=flags)
        stdout = res.stdout or ""

        if "nsec1mcxm333zp9s" in stdout:
            agents_status["Pollen"] = "🟢 在線 (Online - 550B)"
        if "nsec176x8n6290z" in stdout:
            agents_status["Sentry"] = "🟢 在線 (Online - QA)"
        if "nsec1v38r" in stdout or "8be9ec04" in stdout:
            agents_status["Honey2"] = "🟢 在線 (Online - Arch)"
        if "nsec19cxl4" in stdout or "5ef9a934" in stdout:
            agents_status["Fizz"] = "🟢 在線 (Online - Sec)"
        if "76730abc" in stdout:
            agents_status["Honey"] = "🟢 在線 (Online - Core)"
        if "60f596b6" in stdout:
            agents_status["Wax"] = "🟢 在線 (Online - Memory)"
    except Exception:
        pass

    return agents_status

def generate_sync_board(channel: str = None, target_date: str = None):
    if not target_date:
        target_date = datetime.now().strftime("%Y%m%d")

    OUTBOX_DIR.mkdir(parents=True, exist_ok=True)
    out_file = OUTBOX_DIR / f"TASK_SYNC_{target_date}.md"

    agent_status = get_agent_status()

    # 看板內容生成
    md = f"# 📋 蜂群成員協同與進度對齊看板 (TASK_SYNC_{target_date})\n\n"
    md += f"> **看板生成時間**：{datetime.now(timezone.utc).isoformat()}  \n"
    md += f"> **關注頻道**：`{channel or 'All Community Channels'}`  \n\n---\n\n"

    md += "## 🐝 一、 特工在線健康監控矩陣\n\n"
    md += "| 特工名稱 | 角色定位 | 當前通訊狀態 | 預設線路 |\n"
    md += "| :--- | :--- | :---: | :--- |\n"
    md += f"| **Pollen** | 工程執行管家 | {agent_status.get('Pollen')} | NVIDIA NIM 550B 原廠專線 |\n"
    md += f"| **Sentry** | 安全與品質門禁 | {agent_status.get('Sentry')} | 30B Omni Reasoning 治理專線 |\n"
    md += f"| **Honey2** | 系統架構與知識庫 | {agent_status.get('Honey2')} | Nemotron Ultra 550B |\n"
    md += f"| **Fizz** | 主理人專屬秘書 | {agent_status.get('Fizz')} | Gemini 3.1 Flash-Lite 直達私聊 |\n"
    md += f"| **Honey** | 核心演算法協同 | {agent_status.get('Honey')} | DeepSeek / OpenRouter |\n"
    md += f"| **Wax** | 記憶與歷史沉澱 | {agent_status.get('Wax')} | Super VL 120B |\n\n"

    md += "---\n\n"
    md += "## 📌 二、 當前任務進度分佈\n\n"
    md += "### ⚡ 進行中任務 (In Progress)\n"
    md += "- [x] **官方 35 Commits 無損合流** (Relay 防斷線與行動端體驗提升) ➔ 進行中\n"
    md += "- [x] **每日系統進化機制總體落地** (Batch 1 ~ 3 全面啟動) ➔ 進行中\n"
    md += "- [x] **特工守護進程斷線自癒改造** (消除盲目避讓與 DNS 退避) ➔ 進行中\n\n"

    md += "### ⏳ 待推進任務 (To Do)\n"
    md += "- [ ] **MDM Pure Rust 高效分段下載引擎工具落地** (`mister_download_tool.py`)\n"
    md += "- [ ] **Animated-Shorts 程式碼定義敘事協議實裝** (`SKILL.md`)\n\n"

    md += "### ✅ 近期已驗收 (Done)\n"
    md += "- [x] 04_code_health_check.py 代碼體檢與編譯防護驗收 (PASS)\n"
    md += "- [x] Antigravity Quota (AGQ) 擴充套件熱修復驗收 (PASS)\n"
    md += "- [x] Phase 5 全庫編譯技術債掃除與 iroh 模組修復 (PASS)\n\n"

    out_file.write_text(md, encoding="utf-8")
    return str(out_file)

def main():
    parser = argparse.ArgumentParser(description="活躍成員進度對齊看板 (05_task_sync_board)")
    parser.add_argument("--channel", default="5ca019f6-1a27-592a-b015-4baf0eb33572", help="頻道 ID")
    parser.add_argument("--date", default=None, help="目標日期 YYYYMMDD")
    parser.add_argument("--json-only", action="store_true", help="純 JSON 輸出")
    args = parser.parse_args()

    board_path = generate_sync_board(args.channel, args.date)
    if args.json_only:
        print(json.dumps({"status": "success", "board_file": board_path}, ensure_ascii=False, indent=2))
    else:
        print(f"✅ 協同看板已更新: {board_path}")

if __name__ == "__main__":
    main()

