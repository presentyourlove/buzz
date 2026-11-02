#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Honey2::Radar - GitHub 開源專案技術研究員 Sub-Agent

功能概述：
- 每日從 GitHub Trending / Search API 挑選 3-5 個熱門開源專案（最近 7-14 天建立或快速增長）
- 透過 SQLite 本地歷史庫 (tech_research_history.db) 確保「絕不重複研究相同專案」
- 整合 Tavily Search API 抓取專案背景與社群評價
- 使用 Google Gemma 4 31B (google/gemma-4-31b-it:free) 模型進行深度結構化分析：
  1. 用途（有咩用 - 痛點與核心功能）
  2. 用處（用在哪 - 架構定位與生態）
  3. 啱唔啱主理人/Buzz用（契合度與落地建議）
- 產出 Markdown 完整報告儲存於 C:\\Users\\user\\.buzz\\PLANS\\REPORTS\\
- 透過 Buzz Direct Message (DM) 私訊直接推送摘要至主理人專屬私訊通道

維護規範：遵循 Conventional Commits、繁體中文輸出、嚴格錯誤處理與指數退避重試。
"""

import os
import sys
import json
import time
import sqlite3
import argparse
import subprocess
import urllib.request
import urllib.error
from datetime import datetime, timedelta
from typing import List, Dict, Any, Optional

# 常數設定
DEFAULT_MODEL = "google/gemma-4-31b-it:free"
FALLBACK_GEMMA_MODEL = "google/gemma-4-26b-a4b-it:free"
OPENROUTER_URL = "https://openrouter.ai/api/v1/chat/completions"
TAVILY_URL = "https://api.tavily.com/search"
GITHUB_SEARCH_URL = "https://api.github.com/search/repositories"

BASE_BUZZ_DIR = r"C:\Users\user\.buzz"
REPORTS_DIR = os.path.join(BASE_BUZZ_DIR, "PLANS", "REPORTS")
DEFAULT_DB_PATH = os.path.join(BASE_BUZZ_DIR, "tech_research_history.db")
MANAGED_AGENTS_PATH = r"C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\managed-agents.json"
GLOBAL_CONFIG_PATH = r"C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\global-agent-config.json"
BUZZ_CLI_PATH = r"C:\Users\user\AppData\Local\Buzz\buzz.exe"
DEFAULT_RELAY_URL = "wss://presentyourlove.communities.buzz.xyz"
USER_PUBKEY = "2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e"
USER_DM_CHANNEL = "af946a63-dd43-4050-baea-65140d47fc42"


def load_api_keys() -> tuple[str, str]:
    """讀取 OpenRouter 與 Tavily API Key，優先順序：環境變數 -> global-agent-config.json -> .env"""
    openrouter_key = os.environ.get("OPENROUTER_API_KEY", "")
    tavily_key = os.environ.get("TAVILY_API_KEY", "")

    if not openrouter_key or not tavily_key:
        if os.path.exists(GLOBAL_CONFIG_PATH):
            try:
                with open(GLOBAL_CONFIG_PATH, "r", encoding="utf-8") as f:
                    cfg = json.load(f)
                    env_vars = cfg.get("env_vars", {})
                    if not openrouter_key:
                        openrouter_key = env_vars.get("OPENROUTER_API_KEY", "")
                    if not tavily_key:
                        tavily_key = env_vars.get("TAVILY_API_KEY", "")
            except Exception as e:
                print(f"[警告] 讀取 global-agent-config.json 失敗: {e}")

    if not tavily_key:
        env_file = os.path.join(BASE_BUZZ_DIR, ".env")
        if os.path.exists(env_file):
            try:
                with open(env_file, "r", encoding="utf-8") as f:
                    for line in f:
                        if line.startswith("TAVILY_API_KEY="):
                            tavily_key = line.split("=", 1)[1].strip()
                        elif line.startswith("OPENROUTER_API_KEY="):
                            openrouter_key = line.split("=", 1)[1].strip()
            except Exception as e:
                print(f"[警告] 讀取 .env 失敗: {e}")

    return openrouter_key, tavily_key


def load_honey2_credentials() -> tuple[Optional[str], Optional[str]]:
    """從 managed-agents.json 取得 Honey2 的 nsec 私鑰與 auth_tag"""
    if not os.path.exists(MANAGED_AGENTS_PATH):
        print(f"[錯誤] 找不到 managed-agents.json: {MANAGED_AGENTS_PATH}")
        return None, None

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
                if isinstance(auth_tag, list):
                    auth_tag = json.dumps(auth_tag)
                return nsec, auth_tag
    except Exception as e:
        print(f"[錯誤] 解析 managed-agents.json 失敗: {e}")
    return None, None


def init_database(db_path: str = DEFAULT_DB_PATH) -> None:
    """初始化 SQLite 資料庫與去重紀錄表"""
    os.makedirs(os.path.dirname(db_path), exist_ok=True)
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute(
        """
        CREATE TABLE IF NOT EXISTS researched_repos (
            repo_full_name TEXT PRIMARY KEY,
            stars INTEGER,
            language TEXT,
            description TEXT,
            researched_at TEXT,
            report_file TEXT
        )
        """
    )
    conn.commit()
    conn.close()


def is_repo_researched(repo_full_name: str, db_path: str = DEFAULT_DB_PATH) -> bool:
    """檢查專案是否已在 SQLite 歷史中研究過"""
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("SELECT 1 FROM researched_repos WHERE repo_full_name = ?", (repo_full_name,))
    exists = cur.fetchone() is not None
    conn.close()
    return exists


def record_researched_repo(
    repo_full_name: str,
    stars: int,
    language: str,
    description: str,
    report_file: str,
    db_path: str = DEFAULT_DB_PATH
) -> None:
    """將已分析專案記錄至 SQLite"""
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    now_str = datetime.now().isoformat()
    cur.execute(
        """
        INSERT OR REPLACE INTO researched_repos 
        (repo_full_name, stars, language, description, researched_at, report_file)
        VALUES (?, ?, ?, ?, ?, ?)
        """,
        (repo_full_name, stars, language, description, now_str, report_file)
    )
    conn.commit()
    conn.close()


def fetch_trending_repos(limit: int = 3, db_path: str = DEFAULT_DB_PATH) -> List[Dict[str, Any]]:
    """從 GitHub 搜尋近期熱門開源專案，過濾掉已研究過的項目"""
    since_date = (datetime.now() - timedelta(days=14)).strftime("%Y-%m-%d")
    query = f"created:>{since_date}"
    url = f"{GITHUB_SEARCH_URL}?q={query}&sort=stars&order=desc&per_page=20"

    print(f"[1/5] 正在檢索 GitHub 近期趨勢專案 (建立於 {since_date} 之後)...")
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Honey2-Radar-Bot", "Accept": "application/vnd.github.v3+json"}
    )
    
    candidates = []
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            items = data.get("items", [])
            print(f"      共取得 {len(items)} 個候選專案，開始比對 SQLite 去重清單...")
            for item in items:
                name = item.get("full_name")
                if not is_repo_researched(name, db_path):
                    candidates.append({
                        "name": name,
                        "url": item.get("html_url"),
                        "stars": item.get("stargazers_count", 0),
                        "language": item.get("language") or "多語言/其他",
                        "description": item.get("description") or "無描述",
                        "topics": item.get("topics", [])
                    })
                else:
                    print(f"      [跳過已研究] {name}")
                if len(candidates) >= limit:
                    break
    except Exception as e:
        print(f"[錯誤] 呼叫 GitHub API 失敗: {e}")

    return candidates


def search_repo_context_tavily(repo_name: str, tavily_key: str) -> str:
    """使用 Tavily Search API 搜尋專案最新技術背景與社群評價"""
    if not tavily_key:
        return ""
    
    print(f"      正在透過 Tavily 搜尋專案背景資訊: {repo_name}...")
    try:
        payload = {
            "api_key": tavily_key,
            "query": f"github {repo_name} review technical architecture use case",
            "max_results": 2
        }
        req = urllib.request.Request(
            TAVILY_URL,
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"}
        )
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            snippets = [r.get("content", "") for r in data.get("results", [])]
            return "\n".join(snippets)[:1000]
    except Exception as e:
        print(f"      [提醒] Tavily 搜尋非致命錯誤: {e}")
        return ""


def analyze_repo_with_gemma(
    repo: Dict[str, Any],
    context: str,
    openrouter_key: str,
    model: str = DEFAULT_MODEL,
    max_retries: int = 4
) -> str:
    """使用指定 LLM 模型 (預設 google/gemma-4-31b-it:free) 進行三面向深度評估，內建指數退避重試"""
    prompt = f"""請以資深軟體架構師視角，嚴格遵循繁體中文 (Traditional Chinese)，深入分析以下 GitHub 開源專案：

專案名稱：{repo['name']}
專案網址：{repo['url']}
星星數：★{repo['stars']}
主要語言：{repo['language']}
簡短說明：{repo['description']}
主題標籤：{', '.join(repo['topics']) if repo['topics'] else '無'}
搜尋背景脈絡：
{context if context else '無補充背景'}

請針對以下三個維度進行精準分析（條理分明、避免廢話）：
### 1. 用途（有咩用）
- 該專案主要解決了什麼核心痛點？核心功能與技術創新亮點是什麼？

### 2. 用處（用在哪）
- 在實際工程架構中位於哪一層級？適用哪些業務場景？技術棧整合性如何？

### 3. 啱唔啱主理人/Buzz用（契合度與落地評估）
- 評估該專案是否適合 Buzz 系統（去中心化 Nostr 通訊、多代理人 Multi-Agent 協作協同、自主程式碼開發等架構）？
- 是可以直接作為工具集成、或是架構邏輯值得參考借鑒，還是關聯度低僅供觀摩？請給予具體落地建議。
"""

    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": "你是由 Honey2 指派的專屬 GitHub 技術研究員 (Honey2::Radar)。請使用專業、簡潔且道地的繁體中文進行深度開源技術剖析。"},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.3,
        "max_tokens": 1500
    }

    # 嘗試呼叫主模型 (預設 31B)
    models_to_try = [model]
    if model == DEFAULT_MODEL and FALLBACK_GEMMA_MODEL not in models_to_try:
        models_to_try.append(FALLBACK_GEMMA_MODEL)

    for current_model in models_to_try:
        payload["model"] = current_model
        print(f"      正在使用模型 [{current_model}] 深度剖析 {repo['name']}...")
        
        for attempt in range(1, max_retries + 1):
            req = urllib.request.Request(
                OPENROUTER_URL,
                data=json.dumps(payload).encode("utf-8"),
                headers={
                    "Authorization": f"Bearer {openrouter_key}",
                    "Content-Type": "application/json",
                    "HTTP-Referer": "https://github.com/buzz",
                    "X-Title": "Honey2-Radar"
                }
            )
            try:
                with urllib.request.urlopen(req, timeout=40) as resp:
                    result = json.loads(resp.read().decode("utf-8"))
                    content = result["choices"][0]["message"]["content"]
                    return content
            except urllib.error.HTTPError as e:
                err_body = e.read().decode("utf-8", errors="ignore")
                if e.code == 429 or e.code == 500:
                    print(f"      [HTTP {e.code}] 模型 {current_model} 目前 Google 上游節點擁擠/維護中。")
                    if current_model != models_to_try[-1]:
                        print(f"      -> 自動無縫切換至同門姊妹模型 [{models_to_try[-1]}] 繼續執行...")
                        break
                    wait_sec = attempt * 8
                    print(f"      等待 {wait_sec} 秒後進行第 {attempt}/{max_retries} 次重試...")
                    time.sleep(wait_sec)
                else:
                    print(f"      [HTTP 錯誤 {e.code}] {err_body}")
                    break
            except Exception as e:
                print(f"      [請求例外] {e}")
                break

    return f"【評估暫緩】呼叫模型 {model} 遭遇暫時連線異常，請稍後重試。"


def generate_markdown_report(analyses: List[Dict[str, Any]]) -> str:
    """生成結構化 Markdown 研報並保存"""
    today_str = datetime.now().strftime("%Y-%m-%d")
    report_filename = f"{today_str}_GITHUB_TRENDING.md"
    report_path = os.path.join(REPORTS_DIR, report_filename)
    os.makedirs(REPORTS_DIR, exist_ok=True)

    lines = [
        f"# 📡 Honey2::Radar 每日 GitHub 開源專案技術研報",
        f"",
        f"- **研究員身分**：Honey2 專屬 Sub-Agent (`Honey2::Radar`)",
        f"- **產出日期**：{today_str}",
        f"- **核心模型**：`{DEFAULT_MODEL}`",
        f"- **資料來源**：GitHub Trending API + Tavily Search Engine",
        f"- **去重機制**：SQLite Local Database Verified",
        f"",
        f"---",
        f"",
    ]

    for idx, item in enumerate(analyses, 1):
        repo = item["repo"]
        analysis = item["analysis"]
        lines.append(f"## 專案 {idx}：[{repo['name']}]({repo['url']}) (★ {repo['stars']})")
        lines.append(f"- **語言**：`{repo['language']}` | **說明**：{repo['description']}")
        lines.append(f"")
        lines.append(analysis)
        lines.append(f"")
        lines.append(f"---")
        lines.append(f"")

    lines.append(f"### 📋 今日總結與架構師建議")
    lines.append(f"今日精選的 {len(analyses)} 個開源專案已全數歸檔至 SQLite 去重資料庫，明日排程將自動遞延挑選全新項目。")
    lines.append(f"")

    content = "\n".join(lines)
    with open(report_path, "w", encoding="utf-8") as f:
        f.write(content)

    print(f"[4/5] 研報已成功產生並寫入：{report_path}")
    return report_path


def format_repo_summary_for_dm(item: Dict[str, Any]) -> str:
    """將單一專案的三維度分析提煉為精緻且詳盡的私訊展示塊"""
    repo = item["repo"]
    analysis = item.get("analysis", "")

    lines = [
        f"📌 **[{repo['name']}]({repo['url']})** (★{repo['stars']} | {repo['language']})",
        f"  *簡述*：{repo['description']}"
    ]

    parts = {"用途": "", "架構": "", "契合度": ""}
    current_key = None

    for line in analysis.split("\n"):
        line_s = line.strip()
        if "1. 用途" in line_s or "有咩用" in line_s:
            current_key = "用途"
            continue
        elif "2. 用處" in line_s or "用喺邊" in line_s:
            current_key = "架構"
            continue
        elif "3. 啱唔啱" in line_s or "契合度" in line_s:
            current_key = "契合度"
            continue
        elif line_s.startswith("### ") or line_s.startswith("## "):
            current_key = None
            continue

        if current_key and line_s:
            if len(parts[current_key]) < 220:
                clean_text = line_s.lstrip("-*• \t")
                if clean_text:
                    parts[current_key] += (" " if parts[current_key] else "") + clean_text

    if parts["用途"]:
        lines.append(f"  • **核心用途**：{parts['用途'][:160]}...")
    if parts["架構"]:
        lines.append(f"  • **架構定位**：{parts['架構'][:160]}...")
    if parts["契合度"]:
        lines.append(f"  • **主理人/Buzz契合度**：{parts['契合度'][:180]}...")
    elif not parts["用途"]:
        # Fallback：若格式非標準標題，取前 350 字精華摘要
        clean_fallback = analysis.strip().replace("\n", " ")[:350]
        lines.append(f"  • **研報精華**：{clean_fallback}...")

    return "\n".join(lines)


def send_direct_message(report_path: str, analyses: List[Dict[str, Any]]) -> bool:
    """透過 Honey2 憑證發送 Direct Message 私訊給主理人，呈現詳細研讀結論"""
    nsec, auth_tag = load_honey2_credentials()
    if not nsec or not auth_tag:
        print("[錯誤] 無法取得 Honey2 憑證，跳過私訊發送。")
        return False

    today_str = datetime.now().strftime("%Y-%m-%d")
    project_cards = [format_repo_summary_for_dm(item) for item in analyses]
    details_block = "\n\n".join(project_cards)

    dm_content = (
        f"🍯 **【Honey2::Radar 開源專案深度研報完成】**\n\n"
        f"主理人您好！今日精選 {len(analyses)} 項 GitHub 熱門開源技術已研讀完畢，詳細結論如下：\n\n"
        f"{details_block}\n\n"
        f"📄 **完整分析報告存檔**：\n"
        f"`{report_path}`\n\n"
        f"💡 *此訊息由 Honey2 專屬通道直達發送，已自動登記至 SQLite 去重資料庫，保證不重複研讀！*"
    )

    env = os.environ.copy()
    env["BUZZ_RELAY_URL"] = DEFAULT_RELAY_URL
    env["BUZZ_PRIVATE_KEY"] = nsec
    env["BUZZ_AUTH_TAG"] = auth_tag

    cmd = [
        BUZZ_CLI_PATH,
        "messages", "send",
        "--channel", USER_DM_CHANNEL,
        "--content", dm_content
    ]

    print(f"[5/5] 正在發送 Direct Message 私訊至主理人專屬通道 ({USER_DM_CHANNEL})...")
    try:
        res = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=20)
        if res.returncode == 0:
            print("      [成功] Direct Message 已送達主理人視窗！")
            return True
        else:
            print(f"      [警告] DM 發送回傳代碼非 0: {res.stderr}")
            return False
    except Exception as e:
        print(f"      [錯誤] 發送 DM 失敗: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Honey2::Radar - GitHub 開源專案技術研究員")
    parser.add_argument("--run-now", action="store_true", help="立即執行研究與報告產出")
    parser.add_argument("--limit", type=int, default=3, help="挑選之專案數量 (預設 3)")
    parser.add_argument("--no-dm", action="store_true", help="不發送 Direct Message 私訊")
    parser.add_argument("--model", type=str, default=DEFAULT_MODEL, help=f"指定 LLM 模型 (預設 {DEFAULT_MODEL})")
    args = parser.parse_args()

    print("==========================================================")
    print("🐝 Honey2::Radar - GitHub 開源專案技術研究員啟動中")
    print(f"   鎖定模型: {args.model}")
    print("==========================================================")

    init_database()
    openrouter_key, tavily_key = load_api_keys()

    if not openrouter_key:
        print("[致命錯誤] 找不到 OPENROUTER_API_KEY，請確認 global-agent-config.json 或 .env 設定。")
        sys.exit(1)

    repos = fetch_trending_repos(limit=args.limit)
    if not repos:
        print("[提示] 目前沒有符合條件的新專案可供分析。")
        sys.exit(0)

    analyses = []
    for repo in repos:
        print(f"\n[2/5] 正在研讀開源專案: {repo['name']} (★{repo['stars']})")
        context = search_repo_context_tavily(repo["name"], tavily_key)
        analysis_text = analyze_repo_with_gemma(repo, context, openrouter_key, model=args.model)
        analyses.append({"repo": repo, "analysis": analysis_text})

    report_path = generate_markdown_report(analyses)

    # 登記至 SQLite 去重
    for item in analyses:
        record_researched_repo(
            repo_full_name=item["repo"]["name"],
            stars=item["repo"]["stars"],
            language=item["repo"]["language"],
            description=item["repo"]["description"],
            report_file=report_path
        )

    # 發送 DM 私訊
    if not args.no_dm:
        send_direct_message(report_path, analyses)

    print("\n==========================================================")
    print("🎉 Honey2::Radar 本次任務圓滿完成！")
    print(f"   報告路徑: {report_path}")
    print("==========================================================")


if __name__ == "__main__":
    main()
