#!/usr/bin/env python3
"""
Buzz 自動化每日進化分析腳本 (Daily Evolution Script v3.0)

核心升級 (v3.0)：
1. 全景知識回顧：涵蓋「工程頻道 + 主理人 DM 私訊」滾動對話、Obsidian 核心/冷記憶、對話綜述、筆記 (04-Social-Scraps / inbox) 與近期 PLANS
2. 智慧除噪過濾：排除日常非系統諮詢問答與純結案通告，徹底杜絕「拿每日工作總結冒充優化建議」
3. 直連 Honey2 模型：自動從 managed-agents.json 讀取 Honey2 模型配置 (`nvidia/nemotron-3-ultra-550b-a55b:free`) 與延長超時 (90s / 4096 tokens)，避免 JSON 截斷降級
4. 實質進化與雙軌建議：精準拆分「🛡️ 高風險後台架構建議」與「⚡ 低風險特工自主建議」（含根因、優化動作、驗收標準），並輸出「🧬 新技能產出 + 既有技能互相合成」建議
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

try:
    from skill_synthesizer import SkillSynthesizer
except ImportError:
    from scripts.daily_evolution.skill_synthesizer import SkillSynthesizer

try:
    from codebase_verifier import CodebaseVerifier
except ImportError:
    from scripts.daily_evolution.codebase_verifier import CodebaseVerifier

TZ_TAIPEI = timezone(timedelta(hours=8))

DEFAULT_AUTH_TAG = '["auth","2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e","","6f67f68423fe1877d312046cc63cf0106b7f31f284eb51d586dc036a235aeb8433d159d97c37e2b927b40fdc68eff0e1071fdaf56c497494d1779a8bc7fc6118"]'

USER_DM_CHANNEL = "af946a63-dd43-4050-baea-65140d47fc42"
ENGINEERING_CHANNEL = "5ca019f6-1a27-592a-b015-4baf0eb33572"
OBSIDIAN_ROOT = Path(r"C:\私人\ai agent\buzz-obsidian")
MANAGED_AGENTS_PATH = Path(
    os.path.expandvars(r"%APPDATA%\xyz.block.buzz.app\agents\managed-agents.json")
)
GLOBAL_AGENT_CONFIG_PATH = Path(
    os.path.expandvars(r"%APPDATA%\xyz.block.buzz.app\agents\global-agent-config.json")
)

AGENT_NAME_MAP = {
    "2d29f28fd5106795": "主理人",
    "5ef9a934c33b7846": "Fizz",
    "76730abcadc769f9": "Honey",
    "8be9ec04ad41ba97": "Honey2",
    "6f38f58d91639441": "Pollen",
    "8b28b7c24424342d": "Sentry",
    "60f596b66f0ac019": "Wax",
}

OFF_TOPIC_KEYWORDS = [
    "中銀證券",
    "螞蟻銀行",
    "轉倉",
    "ccass",
    "3988.hk",
    "3419.hk",
    "3031.hk",
    "學霸",
    "壓縮軟體",
    "投資與資產問題",
]


class DailyEvolution:
    """每日進化分析與提案生成核心類別"""

    def __init__(
        self,
        channel_id: str,
        relay_url: str,
        private_key: str,
        auth_tag: Optional[str] = None,
        buzz_cli_path: str = "buzz",
        workspace_root: Optional[str] = None,
    ):
        self.channel_id = channel_id
        self.relay_url = relay_url
        self.private_key = private_key
        self.auth_tag = auth_tag or os.environ.get("BUZZ_AUTH_TAG", DEFAULT_AUTH_TAG)
        self.buzz_cli_path = buzz_cli_path
        if workspace_root:
            self.workspace_root = Path(workspace_root)
        else:
            self.workspace_root = Path(__file__).resolve().parent.parent.parent
        self.plans_dir = self.workspace_root / "PLANS"
        self.work_logs_dir = self.workspace_root / "WORK_LOGS"
        self.plans_dir.mkdir(exist_ok=True)
        self.work_logs_dir.mkdir(exist_ok=True)
        self.skill_synthesizer = SkillSynthesizer(workspace_root=str(self.workspace_root))
        self.codebase_verifier = CodebaseVerifier()
        self.active_llm_model = "nvidia/nemotron-3-ultra-550b-a55b:free"

    def _is_production_workspace(self) -> bool:
        """判斷目前是否在正式系統工作區中執行（單元測試使用 temp 目錄時返回 False）"""
        resolved = str(self.workspace_root.resolve()).lower()
        return ".buzz" in resolved or "ai agent\\buzz" in resolved

    def _run_buzz_cli(self, args: List[str]) -> str:
        """執行 Buzz CLI 命令並取得標準輸出 (強制 UTF-8 編碼)"""
        env = os.environ.copy()
        env["BUZZ_RELAY_URL"] = self.relay_url
        env["BUZZ_PRIVATE_KEY"] = self.private_key
        if self.auth_tag:
            env["BUZZ_AUTH_TAG"] = self.auth_tag
        env["PYTHONIOENCODING"] = "utf-8"

        cmd = [self.buzz_cli_path] + args
        flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
        result = subprocess.run(
            cmd, capture_output=True, text=True, env=env, encoding="utf-8", creationflags=flags
        )
        if result.returncode != 0:
            raise RuntimeError(f"Buzz CLI 執行失敗: {result.stderr}")
        return result.stdout

    def post_proposal_to_channel(self, proposal: str) -> str:
        """將精煉的進化建議項目發布至指定頻道"""
        print("📤 正在發布建議進化項目至目標頻道...")
        try:
            env = os.environ.copy()
            env["BUZZ_RELAY_URL"] = self.relay_url
            env["BUZZ_PRIVATE_KEY"] = self.private_key
            if self.auth_tag:
                env["BUZZ_AUTH_TAG"] = self.auth_tag
            env["PYTHONIOENCODING"] = "utf-8"

            cmd = [
                self.buzz_cli_path,
                "messages",
                "send",
                "--channel",
                self.channel_id,
                "--content",
                "-",
            ]
            flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
            result = subprocess.run(
                cmd,
                input=proposal,
                capture_output=True,
                text=True,
                env=env,
                encoding="utf-8",
                creationflags=flags,
            )
            if result.returncode != 0:
                raise RuntimeError(f"發布提案失敗: {result.stderr}")

            try:
                response = json.loads(result.stdout)
                event_id = response.get("event_id", "unknown")
                print(f"   ✅ 建議項目已成功發布至頻道 (事件 ID: {event_id})")
                return event_id
            except json.JSONDecodeError:
                print(f"   ✅ 提案發布完成 (回應內容: {result.stdout[:100]})")
                return "unknown"
        except Exception as e:
            print(f"   ⚠️ 發布提案時發生異常: {e}")
            raise

    def post_summary_to_channel(self, content: str) -> str:
        """相容方法：轉呼叫 post_proposal_to_channel"""
        return self.post_proposal_to_channel(content)

    def fetch_today_messages(self) -> List[Dict[str, Any]]:
        """獲取過去 24 小時內主頻道與主理人私訊 (DM) 之全量對話訊息（依 ID 去重）"""
        now = datetime.now(TZ_TAIPEI)
        start_of_today = datetime(now.year, now.month, now.day, tzinfo=TZ_TAIPEI)
        rolling_24h = now - timedelta(hours=24)
        since_timestamp = int(min(start_of_today, rolling_24h).timestamp())

        channels_to_fetch = [(self.channel_id, "工程頻道")]
        if self._is_production_workspace() and self.channel_id != USER_DM_CHANNEL:
            channels_to_fetch.append((USER_DM_CHANNEL, "主理人私訊"))

        seen_ids = set()
        merged: List[Dict[str, Any]] = []
        for ch_id, ch_label in channels_to_fetch:
            try:
                output = self._run_buzz_cli(
                    [
                        "messages",
                        "get",
                        "--channel",
                        ch_id,
                        "--since",
                        str(since_timestamp),
                        "--limit",
                        "200",
                    ]
                )
                msgs = json.loads(output)
                if isinstance(msgs, list):
                    for m in msgs:
                        mid = m.get("id")
                        if mid and mid in seen_ids:
                            continue
                        if mid:
                            seen_ids.add(mid)
                        m["_channel_label"] = ch_label
                        merged.append(m)
            except Exception as e:
                print(f"警告：讀取 {ch_label} ({ch_id}) 訊息失敗: {e}", file=sys.stderr)

        merged.sort(key=lambda x: x.get("created_at", 0))
        return merged

    def fetch_work_logs(self) -> List[Dict[str, Any]]:
        """讀取 WORK_LOGS/ 目錄下最新紀錄檔"""
        logs = []
        for ext in ("*.md", "*.jsonl"):
            for log_file in sorted(self.work_logs_dir.glob(ext), key=lambda p: p.stat().st_mtime, reverse=True)[:5]:
                try:
                    content = log_file.read_text(encoding="utf-8", errors="ignore")
                    logs.append(
                        {
                            "file": log_file.name,
                            "content": content,
                            "modified": datetime.fromtimestamp(
                                log_file.stat().st_mtime, tz=TZ_TAIPEI
                            ).isoformat(),
                        }
                    )
                except Exception as e:
                    print(f"警告：讀取日誌檔 {log_file} 失敗: {e}", file=sys.stderr)
        return logs

    def fetch_memories_and_notes(self) -> Dict[str, Any]:
        """回顧 Obsidian 核心記憶、冷記憶、對話綜述、前沿筆記 (04-Social-Scraps) 與收件匣靈感"""
        if not self._is_production_workspace() or not OBSIDIAN_ROOT.exists():
            return {"sources": [], "combined_context": ""}

        collected: List[Dict[str, str]] = []
        wiki = OBSIDIAN_ROOT / "wiki"

        scan_targets = [
            (wiki / "00-Core-Memory", "核心記憶", 2, 1800),
            (wiki / "01-Cold-Memory", "冷記憶規格", 5, 1200),
            (wiki / "02-Conversation-Digest" / "Daily", "對話脈絡日報", 3, 1800),
            (wiki / "04-Social-Scraps", "前沿靈感筆記", 6, 1800),
            (OBSIDIAN_ROOT / "inbox", "Inbox靈感", 4, 1200),
        ]

        for folder, label, limit, max_chars in scan_targets:
            if not folder.exists():
                continue
            md_files = sorted(folder.rglob("*.md"), key=lambda p: p.stat().st_mtime, reverse=True)
            for p in md_files[:limit]:
                if p.name.startswith("DAILY_EVOLUTION_PROPOSAL_"):
                    continue
                try:
                    raw = p.read_text(encoding="utf-8", errors="ignore").strip()
                    if raw:
                        collected.append(
                            {
                                "category": label,
                                "name": p.name,
                                "excerpt": raw[:max_chars],
                            }
                        )
                except Exception:
                    continue

        # 補讀特工近期運行日誌中的異常模式（如 7200s timeout 或 503 Overloaded）
        agent_logs_dir = Path(os.path.expandvars(r"%APPDATA%\xyz.block.buzz.app\agents\logs"))
        if agent_logs_dir.exists():
            recent_logs = sorted(agent_logs_dir.glob("*.log"), key=lambda p: p.stat().st_mtime, reverse=True)[:4]
            runtime_anomalies: List[str] = []
            for lf in recent_logs:
                try:
                    tail = "\n".join(lf.read_text(encoding="utf-8", errors="ignore").splitlines()[-120:])
                    for line in tail.splitlines():
                        if any(k in line for k in ("hard turn timeout", "503", "ERROR", "circuit breaker")):
                            runtime_anomalies.append(line.strip()[:160])
                except Exception:
                    continue
            if runtime_anomalies:
                collected.append(
                    {
                        "category": "特工運行異常日誌",
                        "name": "agent_runtime_tail",
                        "excerpt": "\n".join(runtime_anomalies[-8:]),
                    }
                )

        context_blocks = [
            f"[{item['category']} | {item['name']}]\n{item['excerpt'][:700]}"
            for item in collected[:14]
        ]
        return {
            "sources": collected,
            "combined_context": "\n\n---\n".join(context_blocks),
        }

    def _is_closure_or_offtopic(self, text: str) -> bool:
        """過濾已結案完工報告、純禮貌回覆及日常非系統開發諮詢"""
        text_lower = text.lower()
        if any(kw in text_lower for kw in OFF_TOPIC_KEYWORDS):
            return True
        closure_markers = [
            "【phase 1 完成確認】",
            "實作完成交付】",
            "進入待命靜默狀態",
            "保持靜默待命",
            "階段里程碑到達",
            "【qa passed】",
            "【安全與品質驗收通過",
        ]
        return any(m in text_lower for m in closure_markers)

    def analyze_conversations(self, messages: List[Dict], work_logs: List[Dict]) -> Dict[str, Any]:
        """分析對話內容並萃取真正的開發待辦、真實阻礙與共識訊號"""
        author_counts: Dict[str, int] = {}
        topics: List[str] = []
        action_items_raw: List[Dict[str, str]] = []
        blockers_raw: List[Dict[str, str]] = []
        decisions_raw: List[Dict[str, str]] = []
        signal_excerpts: List[str] = []

        noise_patterns = [
            r"^##\s*✅\s*Phase",
            r"^每日系統進化提案",
            r"^【安全與品質驗收",
            r"^###\s*📁\s*新增",
            r"^```json\s*$",
            r"^🍯\s*\*\*【Honey2",
        ]

        for msg in messages:
            raw_content = msg.get("content", "")
            pk_prefix = msg.get("pubkey", "unknown")[:16]
            author_label = AGENT_NAME_MAP.get(pk_prefix, pk_prefix)
            author_counts[author_label] = author_counts.get(author_label, 0) + 1

            lines = raw_content.split("\n")
            filtered_lines = [
                line
                for line in lines
                if not any(re.match(pattern, line.strip()) for pattern in noise_patterns)
            ]
            content = "\n".join(filtered_lines).strip()
            if not content or len(content) < 20:
                continue

            if self._is_closure_or_offtopic(content):
                continue

            content_lower = content.lower()
            ch_label = msg.get("_channel_label", "頻道")
            signal_excerpts.append(f"[{ch_label} | {author_label}] {content[:240].replace(chr(10), ' ')}")

            if any(kw in content_lower for kw in ["bug", "error", "fail", "issue", "problem", "錯誤", "異常", "失敗", "報錯", "timeout", "7200"]):
                topics.append("錯誤與異常分析 (bugs/errors)")
            if any(kw in content_lower for kw in ["feature", "implement", "add", "create", "build", "實作", "功能", "新增"]):
                topics.append("功能開發與演進 (feature development)")
            if any(kw in content_lower for kw in ["review", "pr", "pull request", "merge", "審查", "審核", "驗收", "qa"]):
                topics.append("架構審查與品質驗收 (code review/QA)")
            if any(kw in content_lower for kw in ["deploy", "release", "publish", "發布", "部署", "上線", "熱更新"]):
                topics.append("系統發布與部署 (deployment)")
            if any(kw in content_lower for kw in ["test", "testing", "spec", "測試", "驗證"]):
                topics.append("自動化與整合測試 (testing)")
            if any(kw in content_lower for kw in ["refactor", "cleanup", "improve", "optimize", "重構", "優化", "清理", "記憶", "筆記"]):
                topics.append("架構重構與效能優化 (refactoring/optimization)")

            if any(kw in content_lower for kw in ["todo", "action item", "need to", "should", "must", "待辦", "需要", "必須", "請安排", "排定", "請實作"]):
                action_items_raw.append({"text": content[:200].strip(), "source": f"{author_label}"})

            if any(kw in content_lower for kw in ["blocked", "blocker", "waiting for", "stuck", "阻礙", "卡住", "瓶頸", "無法", "失敗", "exceeded the maximum duration", "overloaded", "503"]):
                blockers_raw.append({"text": content[:200].strip(), "source": f"{author_label}"})

            if any(kw in content_lower for kw in ["decided", "agreed", "approved", "confirmed", "決策", "同意", "確認", "核准", "拍板"]):
                decisions_raw.append({"text": content[:200].strip(), "source": f"{author_label}"})

        topics = sorted(list(set(topics)))

        action_items = [
            {
                "id": f"1.{i}",
                "description": item["text"],
                "source_message": item["source"],
                "priority": "中 (Medium)",
            }
            for i, item in enumerate(action_items_raw[:10], 1)
        ]

        blockers = [
            {
                "id": f"2.{i}",
                "description": item["text"],
                "source_message": item["source"],
                "severity": "高 (High)",
            }
            for i, item in enumerate(blockers_raw[:6], 1)
        ]

        decisions = [
            {
                "id": f"3.{i}",
                "description": item["text"],
                "source_message": item["source"],
                "consensus": True,
            }
            for i, item in enumerate(decisions_raw[:6], 1)
        ]

        total_messages = len(messages)
        author_activity = []
        for author, count in sorted(author_counts.items(), key=lambda x: x[1], reverse=True):
            percentage = round((count / total_messages * 100), 1) if total_messages > 0 else 0
            author_activity.append(
                {
                    "author_prefix": author,
                    "message_count": count,
                    "percentage": percentage,
                }
            )

        work_logs_summary = [
            {
                "file": log["file"],
                "excerpt": log["content"][:250].replace("\n", " ").strip(),
                "modified_at": log["modified"],
            }
            for log in work_logs
        ]

        return {
            "message_count": len(messages),
            "author_activity": author_activity,
            "topics_discussed": topics,
            "action_items": action_items,
            "blockers": blockers,
            "decisions": decisions,
            "signal_excerpts": signal_excerpts[-25:],
            "work_logs_summary": work_logs_summary,
        }

    def fetch_git_changes(self) -> List[str]:
        """抓取近期代碼提交歷史 (Git commits)"""
        commits: List[str] = []
        repos = [
            Path(r"C:\私人\ai agent\buzz"),
            Path(r"C:\私人\ai agent\buzz-obsidian"),
        ]
        for repo in repos:
            if repo.exists() and (repo / ".git").exists():
                try:
                    flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
                    res = subprocess.run(
                        ["git", "-C", str(repo), "log", "-n", "8", "--oneline"],
                        capture_output=True,
                        text=True,
                        encoding="utf-8",
                        timeout=5,
                        creationflags=flags,
                    )
                    for l in res.stdout.splitlines():
                        if l.strip():
                            commits.append(f"[{repo.name}] {l.strip()}")
                except Exception as e:
                    print(f"警告：讀取 Git 倉庫 {repo} 失敗: {e}", file=sys.stderr)
        return commits

    def _load_honey2_llm_config(self) -> Tuple[Optional[str], str]:
        """直接從 managed-agents.json 載入 Honey2 使用的模型與 OpenRouter API Key"""
        api_key = os.environ.get("OPENROUTER_API_KEY")
        model = "nvidia/nemotron-3-ultra-550b-a55b:free"

        if MANAGED_AGENTS_PATH.exists():
            try:
                agents = json.loads(MANAGED_AGENTS_PATH.read_text(encoding="utf-8"))
                honey2 = next(
                    (
                        a
                        for a in agents
                        if a.get("name") == "Honey2"
                        and a.get("is_active") is True
                        and a.get("env_vars", {}).get("OPENROUTER_API_KEY")
                    ),
                    None,
                )
                if not honey2:
                    honey2 = next(
                        (a for a in agents if a.get("name") == "Honey2" and a.get("is_active") is True),
                        None,
                    )
                if honey2:
                    if honey2.get("model"):
                        model = honey2["model"]
                    if not api_key:
                        api_key = honey2.get("env_vars", {}).get("OPENROUTER_API_KEY")
            except Exception as e:
                print(f"讀取 managed-agents.json Honey2 配置異常: {e}", file=sys.stderr)

        if GLOBAL_AGENT_CONFIG_PATH.exists():
            try:
                cfg = json.loads(GLOBAL_AGENT_CONFIG_PATH.read_text(encoding="utf-8"))
                if not api_key:
                    api_key = cfg.get("env_vars", {}).get("OPENROUTER_API_KEY")
                if not model and cfg.get("model"):
                    model = cfg["model"]
            except Exception:
                pass

        return api_key, model

    def _call_openrouter_llm(self, prompt: str) -> Optional[str]:
        """直接調用 Honey2 專屬模型 (`nvidia/nemotron-3-ultra-550b-a55b:free`) 進行深度進化分析"""
        api_key, honey2_model = self._load_honey2_llm_config()
        self.active_llm_model = honey2_model
        if not api_key:
            return None

        candidate_models = [honey2_model]
        for fallback in ("openrouter/free", "nvidia/nemotron-3-nano-30b-a3b:free"):
            if fallback not in candidate_models:
                candidate_models.append(fallback)

        system_prompt = (
            "你是 Buzz 多特工系統的架構進化企劃官 Honey2。"
            "你的職責是綜合分析【團隊真實對話】、【Obsidian 核心/冷記憶與筆記】與【運行異常日誌】，"
            "研究出真正具備前瞻性與可行性的『系統進化優化建議』以及『技能產出/互相合成建議』。\n"
            "【嚴格禁止】：絕對不可輸出『今日完成確認』、『誰做了幾輪』等工作總結！每條建議必須是能讓系統變得更強、解決真實瓶頸的具體優化方案！"
            "【輸出限制】：全程使用繁體中文，嚴禁在路徑或變數名稱使用角括號（請改用反引號）。"
        )

        for model in candidate_models:
            for attempt in range(2):
                try:
                    payload = json.dumps(
                        {
                            "model": model,
                            "messages": [
                                {"role": "system", "content": system_prompt},
                                {"role": "user", "content": prompt},
                            ],
                            "temperature": 0.35,
                            "max_tokens": 4096,
                        }
                    ).encode("utf-8")

                    req = urllib.request.Request(
                        "https://openrouter.ai/api/v1/chat/completions",
                        headers={
                            "Authorization": f"Bearer {api_key}",
                            "Content-Type": "application/json",
                            "HTTP-Referer": "https://buzz.local",
                            "X-Title": "Buzz-Honey2-Daily-Evolution",
                        },
                        data=payload,
                    )
                    with urllib.request.urlopen(req, timeout=90) as resp:
                        data = json.loads(resp.read().decode("utf-8"))
                        msg = data["choices"][0]["message"]
                        content = msg.get("content") or msg.get("reasoning") or ""
                        if content and len(content.strip()) > 40:
                            self.active_llm_model = model
                            return content
                except urllib.error.HTTPError as he:
                    print(f"Honey2 模型 {model} HTTP {he.code} (第 {attempt + 1} 次): {he.reason}", file=sys.stderr)
                    if he.code in (429, 503) and attempt == 0:
                        time.sleep(2)
                        continue
                    break
                except Exception as e:
                    print(f"Honey2 模型 {model} 呼叫異常 (第 {attempt + 1} 次): {e}", file=sys.stderr)
                    if attempt == 0:
                        time.sleep(1)
                        continue
                    break
        return None

    def _extract_json_block(self, text: str) -> Optional[Dict[str, Any]]:
        """從 LLM 回應中穩定抽取 JSON 物件（容錯思考標籤或 Markdown 圍欄）"""
        clean = text.strip()
        fenced = re.search(r"```(?:json)?\s*(\{.*?\})\s*```", clean, flags=re.DOTALL)
        if fenced:
            try:
                return json.loads(fenced.group(1))
            except Exception:
                pass
        start = clean.find("{")
        end = clean.rfind("}")
        if start != -1 and end > start:
            try:
                return json.loads(clean[start : end + 1])
            except Exception:
                pass
        return None

    def generate_dynamic_recommendations(
        self,
        analysis: Dict[str, Any],
        git_commits: List[str],
        memories_info: Optional[Dict[str, Any]] = None,
        installed_skills: Optional[Dict[str, Dict[str, str]]] = None,
    ) -> Dict[str, Any]:
        """透過 Honey2 模型綜合對話、記憶與筆記，生成高/低風險系統進化建議與技能合成創意"""
        memories_ctx = (memories_info or {}).get("combined_context", "")[:4500]
        installed_list = (
            ", ".join(sorted(installed_skills.keys())) if installed_skills else "obsidian-digest-sync, browser-automation, qa-guard, env-guardian, daily-digest, alpha-research, trend-following, var-models"
        )

        context_lines = [
            f"【討論主題】：{', '.join(analysis.get('topics_discussed', [])) or '架構調研與功能演進'}",
        ]
        if analysis.get("signal_excerpts"):
            context_lines.append("【近期關鍵對話摘錄（含工程頻道與主理人私訊）】：")
            context_lines.extend(f" - {s}" for s in analysis["signal_excerpts"][-12:])
        if analysis.get("blockers"):
            context_lines.append("【系統真實阻礙與報錯訊號】：")
            context_lines.extend(f" - {b['description'][:150]}" for b in analysis["blockers"][:5])
        if git_commits:
            context_lines.append("【近期 Git 代碼異動】：")
            context_lines.extend(f" - {c}" for c in git_commits[:6])
        if memories_ctx:
            context_lines.append("【Obsidian 核心記憶、冷記憶與前沿筆記 (04-Social-Scraps / inbox)】：")
            context_lines.append(memories_ctx)

        context_lines.append(f"【目前系統已安裝技能庫】：{installed_list}")

        prompt = (
            "請仔細閱讀以下【對話紀錄】、【記憶庫與前沿筆記】以及【已安裝技能庫】，進行深度架構研究。\n"
            "【要求】：\n"
            "1. 產出真正能讓 Buzz 變得更好用的 **系統進化優化建議**（絕對不可寫成已完成工作總結！每項建議須寫明優化動作與驗收標準）。\n"
            "2. 區分為兩層：\n"
            "   - `high_risk_arch` (2-3項)：高風險／系統級後台架構建議（如 Rust 核心、`buzz-acp` 守護、進程隔離、API 重試鏈、Git 雙軌）。\n"
            "   - `low_risk_agent` (2-3項)：低風險／應用級特工進化建議（如工作流、Prompt 紀律、Obsidian 筆記消化、自動化工具、測試補齊）。\n"
            "3. 研究現有已安裝技能與近期需求，產出 1-2 項 `skill_suggestions`：\n"
            "   - `kind`: `new`（從近期對話/記憶提煉全新技能）或 `synthesis`（將 2 個以上現有技能互相合成為新技能，務必在 `base_skills` 列出基底技能）。\n\n"
            + "\n".join(context_lines)
            + '\n\n請僅回傳純 JSON，不要有任何多餘說明：\n'
            '{\n'
            '  "high_risk_arch": [\n'
            '    {"id": "A1", "description": "針對[具體瓶頸/筆記靈感]，建議[具體架構優化動作]（驗收：[如何驗證成功]）", "source": "記憶/日誌研究"}\n'
            '  ],\n'
            '  "low_risk_agent": [\n'
            '    {"id": "B1", "description": "針對[具體應用場景]，建議[具體優化動作]（驗收：[如何驗證成功]）", "source": "對話/筆記研究"}\n'
            '  ],\n'
            '  "skill_suggestions": [\n'
            '    {\n'
            '      "kind": "synthesis",\n'
            '      "name": "kebab-case-skill-name",\n'
            '      "title": "繁體中文技能標題",\n'
            '      "base_skills": ["skill-a", "skill-b"],\n'
            '      "description": "一句話說明此技能如何解決痛點或融合既有能力",\n'
            '      "reason": "為何從今日記憶/筆記建議產出或合成",\n'
            '      "triggers": ["觸發條件1"],\n'
            '      "steps": ["步驟1", "步驟2", "步驟3"]\n'
            '    }\n'
            '  ]\n'
            '}'
        )

        llm_output = self._call_openrouter_llm(prompt)
        if llm_output:
            parsed = self._extract_json_block(llm_output)
            if parsed and ("high_risk_arch" in parsed or "process_improvements" in parsed):
                high_items = parsed.get("high_risk_arch", [])
                low_items = parsed.get("low_risk_agent", [])
                for it in high_items:
                    it["risk_tier"] = "high"
                    it.setdefault("checked", False)
                for it in low_items:
                    it["risk_tier"] = "low"
                    it.setdefault("checked", False)
                print(f"✅ 成功透過 Honey2 ({self.active_llm_model}) 生成深度系統進化與技能合成建議！")
                return {
                    "high_risk_arch": high_items,
                    "low_risk_agent": low_items,
                    "process_improvements": low_items[:2],
                    "technical_debt": high_items[:2],
                    "team_coordination": low_items[2:] or low_items[:1],
                    "automation_opportunities": high_items[1:] or low_items[:1],
                    "skill_suggestions": parsed.get("skill_suggestions", []),
                }

        print("💡 LLM 離線或未回傳合法 JSON，啟動深度記憶脈絡推演引擎（杜絕總結型假建議）...")
        return self._generate_contextual_fallback_recommendations(analysis, memories_info)

    def _generate_contextual_fallback_recommendations(
        self, analysis: Dict[str, Any], memories_info: Optional[Dict[str, Any]]
    ) -> Dict[str, Any]:
        """當 AI 通道暫時離線時，根據真實日誌痛點與記憶筆記產出具體優化建議（嚴禁回傳半截總結）"""
        high_risk: List[Dict[str, Any]] = []
        low_risk: List[Dict[str, Any]] = []

        blockers_text = " ".join(b.get("description", "") for b in analysis.get("blockers", [])).lower()
        signals_text = " ".join(analysis.get("signal_excerpts", [])).lower()
        mem_text = (memories_info or {}).get("combined_context", "").lower()

        if "7200" in blockers_text or "maximum duration" in blockers_text or "7200" in mem_text:
            high_risk.append(
                {
                    "id": "A1",
                    "risk_tier": "high",
                    "description": "針對特工大型開發撞上 `buzz-acp` 7200 秒硬性超時限制，建議在 `managed-agents.json` 為 Pollen 支援分段超時調優或導入單回合 60 分鐘階段存檔交棒機制（驗收：超長任務能安全存檔接續不遭 dead-letter 中斷）",
                    "source": "運行日誌 RCA",
                    "checked": True,
                }
            )
        if "503" in blockers_text or "overloaded" in blockers_text or "503" in mem_text:
            high_risk.append(
                {
                    "id": "A2",
                    "risk_tier": "high",
                    "description": "針對 OpenRouter `503 Overloaded` 波動導致特工重試失敗，建議強化本地思考代理的多通道跨供應商指數退避與備援模型自動降級（驗收：503 觸發時自動轉走無感完成回應）",
                    "source": "API 穩定性研究",
                    "checked": False,
                }
            )
        if len(high_risk) < 2:
            high_risk.append(
                {
                    "id": f"A{len(high_risk) + 1}",
                    "risk_tier": "high",
                    "description": "針對已通過單元測試的 Proactive Triggers 模組，建議於執行時期掛載真實定時喚醒排程並驗證關閉時無 Tokio 孤兒任務殘留（驗收：`cargo test -p buzz-agent` 整合測試全數通過且無殘留行程）",
                    "source": "冷記憶與架構推演",
                    "checked": False,
                }
            )
        if len(high_risk) < 2:
            high_risk.append(
                {
                    "id": "A2",
                    "risk_tier": "high",
                    "description": "維護雙軌 Git 分支純淨合流機制，將日常開發鎖定於 `feature/custom-local-core` 並定期快進 `main` 鏡像，確保每次官方更新一鍵可見、零衝突升級（驗收：Git Graph 雙軌分明、0 未預期衝突）",
                    "source": "工程治理規範",
                    "checked": False,
                }
            )

        if "always-on" in signals_text or "always-on" in mem_text or "記憶" in signals_text:
            low_risk.append(
                {
                    "id": "B1",
                    "risk_tier": "low",
                    "description": "結合筆記 `anthropic-always-on-agents.md` 與核心記憶架構，為各特工建立獨立 `memory_namespace` 隔離熱記憶與冷記憶索引，避免長駐執行造成脈絡污染（驗收：特工狀態查詢與跨串記憶召回互不干擾）",
                    "source": "筆記與記憶整合",
                    "checked": True,
                }
            )
        else:
            low_risk.append(
                {
                    "id": "B1",
                    "risk_tier": "low",
                    "description": "建立「從 Obsidian `04-Social-Scraps` 與 `inbox/` 主動消化筆記」的週期性工作流，每晚自動擷取未落實的前沿靈感轉為 Buzz 系統改進提案（驗收：inbox 新筆記能在當晚進化提案中被精準引用）",
                    "source": "第二大腦自動化",
                    "checked": False,
                }
            )

        low_risk.append(
            {
                "id": "B2",
                "risk_tier": "low",
                "description": "統一規範 Pollen 在超過 80 輪的重型任務中採取「寫完一個子模組即執行測試＋落盤回報」的里程碑節奏，降低上下文膨脹與中途遺失風險（驗收：單次回合壓縮次數下降且 QA 一次通過率提升）",
                "source": "特工協作優化",
                "checked": False,
            }
        )

        return {
            "high_risk_arch": high_risk[:2],
            "low_risk_agent": low_risk[:2],
            "process_improvements": low_risk[:2],
            "technical_debt": high_risk[:2],
            "team_coordination": [low_risk[0]],
            "automation_opportunities": [high_risk[0]],
            "skill_suggestions": [],
        }

    def generate_proposal(
        self,
        analysis: Dict[str, Any],
        date_str: str,
        skill_candidates: Optional[List[Dict[str, Any]]] = None,
        recommendations: Optional[Dict[str, Any]] = None,
        memories_info: Optional[Dict[str, Any]] = None,
    ) -> Tuple[str, str]:
        """生成完整備查提案 (PLANS/) 與極簡雙層速讀行動卡片 (DM/頻道)"""
        now = datetime.now(TZ_TAIPEI)
        if skill_candidates is None:
            skill_candidates = []

        if recommendations is None:
            git_commits = self.fetch_git_changes()
            recommendations = self.generate_dynamic_recommendations(
                analysis, git_commits, memories_info=memories_info
            )

        high_risk = recommendations.get("high_risk_arch", [])
        low_risk = recommendations.get("low_risk_agent", [])
        if not high_risk:
            high_risk = recommendations.get("technical_debt", [])
        if not low_risk:
            low_risk = recommendations.get("process_improvements", [])

        topics_list = (
            "\n".join(f"- {t}" for t in analysis["topics_discussed"])
            if analysis.get("topics_discussed")
            else "- 系統記憶整合與持續演進"
        )

        action_items_list = (
            "\n".join(
                f"{item['id']}. {item['description']} *(來源：`{item['source_message']}`)*"
                for item in analysis["action_items"]
            )
            if analysis.get("action_items")
            else "本日對話已整合入深度進化建議，無額外零散待辦。"
        )

        blockers_list = (
            "\n".join(
                f"{item['id']}. {item['description']} *(來源：`{item['source_message']}`)*"
                for item in analysis["blockers"]
            )
            if analysis.get("blockers")
            else "本日系統執行順暢，未偵測到阻斷性故障。"
        )

        work_logs_summary = (
            "\n".join(
                f"- **`{log['file']}`** (更新於 {log['modified_at'][:19]}): {log['excerpt']}..."
                for log in analysis["work_logs_summary"]
            )
            if analysis.get("work_logs_summary")
            else "本日暫無新增工作日誌。"
        )

        memories_list = (
            "\n".join(
                f"- **[{m['category']}]** `{m['name']}`"
                for m in (memories_info or {}).get("sources", [])[:12]
            )
            if memories_info and memories_info.get("sources")
            else "- 本次執行於獨立測試沙箱環境，未掛載外部 Obsidian 筆記庫。"
        )

        codebase_report = self.codebase_verifier.generate_verification_report_markdown()
        implemented_items = analysis.get("implemented_items", [])
        implemented_section = (
            "\n".join(
                f"- ✅ **{item.get('matched_feature', item.get('description'))}** (狀態：{item.get('status', '已實作驗證通過')})"
                for item in implemented_items
            )
            if implemented_items
            else "本日既有模組驗證維持綠燈。"
        )

        if skill_candidates:
            skill_candidates_full = "\n".join(
                f"- [ ] **{'🔗 技能合成' if sc.get('kind') == 'synthesis' else '🆕 新技能產出'} `{sc['name']}`** — {sc['title']}  \n"
                + (
                    f"  - **合成配方**：`{' + '.join(sc['base_skills'])}` ➔ `{sc['name']}`  \n"
                    if sc.get("base_skills")
                    else ""
                )
                + f"  - **說明**：{sc['description']}  \n"
                f"  - **提煉理由**：{sc.get('synthesis_reason', sc.get('source_summary', ''))}  \n"
                f"  - **候選檔案**：`{sc['path']}`  \n"
                f"  - **批准指令**：回覆 `同意技能: {sc['name']}`"
                for sc in skill_candidates
            )
        else:
            skill_candidates_full = "本日所有識別之高價值流程與複合技能皆已在庫生效。"

        metadata = {
            "template_version": "3.0",
            "llm_model": self.active_llm_model,
            "channel_id": self.channel_id,
            "date": date_str,
            "message_count": analysis["message_count"],
            "memories_reviewed": len((memories_info or {}).get("sources", [])),
            "skill_candidates_count": len(skill_candidates),
            "generated_at": now.isoformat(),
            "script_version": "daily_evolution.py v3.0 (Honey2 Deep Research + Skill Synthesis)",
        }

        full_proposal = f"""---
title: "每日系統進化提案 - {date_str}"
tags: [daily-evolution, optimization, skill-synthesis, automated]
status: pending-approval
created: {date_str}
version: "3.0"
---

# 每日系統進化提案 - {date_str}

> **模板版本**：3.0 (全景記憶研究 + Honey2 模型驅動 + 技能產出/互相合成)  
> **推理引擎**：`{self.active_llm_model}`  
> **生成日期**：`{date_str}` (Asia/Taipei)

---

## 📊 本日活動與知識庫回顧摘要 (Daily & Memory Summary)

| 指標名稱 | 統計數值 |
|----------|----------|
| **主分析頻道** | `{self.channel_id}` (含主理人 DM) |
| **分析訊息數** | {analysis['message_count']} 則 |
| **回顧記憶與筆記數** | {len((memories_info or {}).get('sources', []))} 份 |
| **技能產出／合成候選** | {len(skill_candidates)} 項 |
| **分析時間戳記** | {now.strftime('%Y-%m-%d %H:%M:%S')} (UTC+8) |

### 成員活躍統計
| 成員名稱 | 發言則數 | 佔比 |
|----------|----------|------|
{"".join(f"| `{a['author_prefix']}` | {a['message_count']} | {a['percentage']}% |\n" for a in analysis['author_activity'])}
### 回顧記憶與筆記清單
{memories_list}

### 討論主題
{topics_list}

---

## 🎯 偵測之待辦脈絡與系統瓶頸
### 待辦訊號
{action_items_list}

### 阻礙與風險訊號
{blockers_list}

---

## 📝 工作日誌摘要
{work_logs_summary}

---

{codebase_report}

### ✅ 反幻覺過濾（已實作完成項目）
{implemented_section}

---

## 💡 系統進化優化建議 (Optimization Recommendations)

### 🛡️ 1. 後台架構進化建議（高風險 / 系統級 — 建議 Antigravity 執行）
{"".join(f"- [ ] **A{i+1}.** {item['description']} *(來源：{item.get('source', '架構研究')})*\n" for i, item in enumerate(high_risk[:3]))}
### ⚡ 2. 特工自主進化建議（低風險 / 應用級 — 可授權 Pollen / Honey2 執行）
{"".join(f"- [ ] **B{i+1}.** {item['description']} *(來源：{item.get('source', '對話與記憶研究')})*\n" for i, item in enumerate(low_risk[:3]))}
### 🧬 3. 技能產出與互相合成候選 (Skill Generation & Synthesis)
{skill_candidates_full}

---

## 📋 老闆審核機制 (Approval Required)

| 回覆指令 | 觸發動作 |
|----------|----------|
| `1` / `同意特工` | 授權 Pollen / Honey2 實作低風險特工進化項目 |
| `2` / `交由後台` | 委派 Antigravity 於後台處理高風險系統架構升級 |
| `同意技能: 技能名稱` | 審核通過特定新技能或合成技能，交由 Sentry 檢驗並雙軌落庫啟用 |
| `0` / `暫緩` | 暫不處理，全數歸檔 |

---

## 📎 結構化元數據 (Metadata)

```json
{json.dumps(metadata, indent=2, ensure_ascii=False)}
```

*由 Honey2 每日進化系統 v3.0 於 {now.strftime('%Y-%m-%d %H:%M:%S')} (UTC+8) 自動生成*
"""

        arch_section = "\n".join(
            f"- **A{i+1}.** {it['description']}" for i, it in enumerate(high_risk[:2])
        )
        agent_section = "\n".join(
            f"- **B{i+1}.** {it['description']}" for i, it in enumerate(low_risk[:2])
        )

        new_cands = [c for c in skill_candidates if c.get("kind") != "synthesis"]
        synth_cands = [c for c in skill_candidates if c.get("kind") == "synthesis"]

        skill_lines: List[str] = []
        if new_cands:
            c = new_cands[0]
            skill_lines.append(
                f"- 🆕 **新技能產出 `{c['name']}`**：{c.get('synthesis_reason') or c.get('description', '')[:100]}"
            )
        if synth_cands:
            c = synth_cands[0]
            formula = f"`{' + '.join(c['base_skills'])}` ➔ " if c.get("base_skills") else ""
            skill_lines.append(
                f"- 🔗 **技能互相合成 {formula}`{c['name']}`**：{c.get('synthesis_reason') or c.get('description', '')[:100]}"
            )
        if len(skill_lines) < 2 and len(synth_cands) > 1:
            c = synth_cands[1]
            formula = f"`{' + '.join(c['base_skills'])}` ➔ " if c.get("base_skills") else ""
            skill_lines.append(
                f"- 🔗 **技能互相合成 {formula}`{c['name']}`**：{c.get('synthesis_reason') or c.get('description', '')[:100]}"
            )

        if skill_lines:
            skill_section = "\n🧬 **【技能產出與互相合成建議 (Skill Generation & Synthesis)】**\n" + "\n".join(skill_lines) + "\n"
        else:
            skill_section = "\n🧬 **【技能產出與互相合成建議】**\n- 本日現有高價值技能皆已落庫生效，暫無新增候選。\n"

        reviewed_mem_count = len((memories_info or {}).get("sources", []))
        channel_proposal = f"""🍯 **【Honey2 每日系統進化速讀卡片】** (`{date_str}`)
> 📌 **代碼庫健康驗證**：已回顧 `{analysis['message_count']}` 則對話與 `{reviewed_mem_count}` 份記憶／筆記（引擎：`{self.active_llm_model}` ✅）

🛡️ **【後台架構進化建議（高風險 / 系統級）】** *(由 Antigravity 於後台執行，不佔特工回合)*
{arch_section}
  👉 *負責人：`Antigravity` 於後台終端機執行*

⚡ **【特工自主進化建議（低風險 / 應用級）】** *(授權後可由 Pollen / Honey2 自主實作)*
{agent_section}
  👉 *負責人：`Pollen` / `Honey2` 自主完成並提交 Sentry 驗收*
{skill_section}
──────────────────────────────────────────
💡 **主理人快捷批示** (直接在對話中回覆)：
• 回覆 `1` 或 `同意特工` → 授權 Pollen 開工低風險進化建議
• 回覆 `2` 或 `交由後台` → 委由 Antigravity 於後台執行高風險系統架構維護
• 回覆 `同意技能: 技能名稱` → 交由 Sentry 審核並啟用上方新產出或合成技能
• 回覆 `0` 或 `暫緩` → 暫不處理，僅歸檔備查
📄 **完整進化研報存檔**：`PLANS/DAILY_EVOLUTION_PROPOSAL_{date_str.replace('-', '')}.md`"""

        return full_proposal, channel_proposal

    def save_proposal(self, proposal: str, date_str: str) -> Path:
        """將完整提案儲存至 PLANS/、Obsidian 知識庫與 Wax 廣播日誌"""
        filename = f"DAILY_EVOLUTION_PROPOSAL_{date_str.replace('-', '')}.md"
        filepath = self.plans_dir / filename
        filepath.write_text(proposal, encoding="utf-8")

        if self._is_production_workspace():
            try:
                obsidian_dir = OBSIDIAN_ROOT / "wiki" / "02-Conversation-Digest"
                if obsidian_dir.exists():
                    (obsidian_dir / filename).write_text(proposal, encoding="utf-8")
            except Exception as e:
                print(f"   ⚠️ 同步至 Obsidian 失敗 (非致命): {e}")

            try:
                wax_log = Path(r"C:\Users\user\.buzz\PLANS\WAX_BROADCAST_LOG.md")
                wax_log.parent.mkdir(parents=True, exist_ok=True)
                with open(wax_log, "a", encoding="utf-8") as f:
                    f.write(
                        f"\n\n---\n<!-- DAILY_EVOLUTION_{date_str} -->\n### 📅 [每日進化提案自動產出] - {date_str}\n- **提案檔案**: `{filename}`\n- **儲存路徑**: `{filepath}`\n- **狀態**: 已就緒，等待審核\n"
                    )
            except Exception as e:
                print(f"   ⚠️ 寫入 Wax 廣播日誌失敗 (非致命): {e}")

        return filepath

    def generate_summary(
        self,
        analysis: Dict[str, Any],
        proposal_path: Path,
        date_str: str,
        skill_candidates: Optional[List[Dict[str, Any]]] = None,
    ) -> str:
        """生成繁體中文簡要摘要"""
        candidates_count = len(skill_candidates) if skill_candidates else 0
        return f"""## 📅 每日系統進化提案 - {date_str}

**系統自動生成之優化提案已就緒，等待審核。**

### 📊 本日活動統計
- **分析訊息**：{analysis['message_count']} 則
- **活躍成員**：{len(analysis['author_activity'])} 位
- **技能產出／合成候選**：{candidates_count} 項

### 📄 完整計畫書
已存檔至：`{proposal_path.name}`

*由 Honey2 每日進化系統 v3.0 自動生成*"""

    def run(self, post_to_channel: bool = True) -> Dict[str, Any]:
        """執行完整每日進化分析、技能合成與推播流程"""
        now = datetime.now(TZ_TAIPEI)
        date_str = now.strftime("%Y-%m-%d")
        print(f"🔄 正在啟動 {date_str} 每日進化分析流程 v3.0 (時區: Asia/Taipei)...")

        print("📥 正在讀取工程頻道與主理人私訊對話...")
        messages = self.fetch_today_messages()
        print(f"   共讀取到 {len(messages)} 則跨頻道訊息")

        print("📖 正在讀取工作日誌...")
        work_logs = self.fetch_work_logs()
        print(f"   共讀取到 {len(work_logs)} 份日誌檔案")

        print("🧠 正在回顧 Obsidian 核心記憶、冷記憶、對話綜述與前沿筆記...")
        memories_info = self.fetch_memories_and_notes()
        print(f"   共回顧 {len(memories_info.get('sources', []))} 份記憶與筆記")

        print("🌿 正在讀取近期 Git 提交歷史...")
        git_commits = self.fetch_git_changes()
        print(f"   共讀取到 {len(git_commits)} 筆提交紀錄")

        print("🔍 正在進行對話去噪與痛點萃取...")
        analysis = self.analyze_conversations(messages, work_logs)

        print("🛡️ 正在進行代碼庫 AST 符號與反幻覺校準...")
        calibrated_actions, calibrated_blockers, implemented_items = self.codebase_verifier.calibrate_recommendations(
            analysis
        )
        analysis["action_items"] = calibrated_actions
        analysis["blockers"] = calibrated_blockers
        analysis["implemented_items"] = implemented_items
        print(f"   已校準並剔除 {len(implemented_items)} 項已實作特性")

        installed_skills = self.skill_synthesizer.scan_installed_skills_detail()

        print("✨ 正在調用 Honey2 模型進行深度進化研究...")
        recommendations = self.generate_dynamic_recommendations(
            analysis,
            git_commits,
            memories_info=memories_info,
            installed_skills=installed_skills,
        )

        print("🧬 正在執行新技能產出與既有技能互相合成...")
        extra_texts = [{"source": "ConversationSignals", "text": "\n".join(analysis.get("signal_excerpts", []))}]
        skill_candidates: List[Dict[str, Any]] = []
        try:
            skill_candidates = self.skill_synthesizer.detect_and_synthesize(
                extra_texts=extra_texts,
                llm_skill_ideas=recommendations.get("skill_suggestions"),
                include_memories_and_notes=self._is_production_workspace(),
            )
            print(f"   共產出／合成 {len(skill_candidates)} 項技能候選")
        except Exception as e:
            print(f"   ⚠️ 技能合成過程發生異常 (繼續執行): {e}")

        print("📝 正在組裝進化研報與極簡速讀行動卡片...")
        full_proposal, channel_proposal = self.generate_proposal(
            analysis,
            date_str,
            skill_candidates=skill_candidates,
            recommendations=recommendations,
            memories_info=memories_info,
        )

        print("💾 正在儲存完整提案檔案至 PLANS/ 與 Obsidian...")
        proposal_path = self.save_proposal(full_proposal, date_str)
        print(f"   已存檔至: {proposal_path}")

        summary = self.generate_summary(
            analysis, proposal_path, date_str, skill_candidates=skill_candidates
        )

        event_id = None
        if post_to_channel:
            try:
                event_id = self.post_proposal_to_channel(channel_proposal)
            except Exception as e:
                print(f"   ⚠️ 發布至頻道時發生異常 (繼續執行): {e}")

        print("✅ 每日進化分析流程 v3.0 執行完畢！")
        return {
            "date": date_str,
            "proposal_path": str(proposal_path),
            "summary": summary,
            "channel_proposal": channel_proposal,
            "analysis": analysis,
            "skill_candidates": skill_candidates,
            "posted_event_id": event_id,
        }


def main():
    parser = argparse.ArgumentParser(description="Buzz 每日進化分析腳本")
    parser.add_argument("--channel", required=True, help="目標頻道 UUID")
    parser.add_argument("--relay-url", required=True, help="Buzz Relay 網址")
    parser.add_argument("--private-key", required=True, help="身分私鑰 (hex 或 nsec)")
    parser.add_argument("--auth-tag", help="NIP-OA 頻道認證標籤 JSON")
    parser.add_argument("--buzz-cli", default="buzz", help="Buzz CLI 執行檔路徑")
    parser.add_argument("--workspace", help="工作區根目錄")
    parser.add_argument("--dry-run", action="store_true", help="演練模式")
    parser.add_argument("--no-post", action="store_true", help="儲存提案但不發送頻道")

    args = parser.parse_args()

    evolution = DailyEvolution(
        channel_id=args.channel,
        relay_url=args.relay_url,
        private_key=args.private_key,
        auth_tag=args.auth_tag,
        buzz_cli_path=args.buzz_cli,
        workspace_root=args.workspace,
    )

    try:
        post_to_channel = not args.dry_run and not args.no_post
        result = evolution.run(post_to_channel=post_to_channel)

        if args.dry_run:
            print("\n--- 演練模式：頻道建議項目 ---")
            print(result["channel_proposal"])
        else:
            print(
                json.dumps(
                    {
                        "success": True,
                        "summary": result["summary"],
                        "proposal_path": result["proposal_path"],
                        "posted_event_id": result.get("posted_event_id"),
                    },
                    ensure_ascii=False,
                )
            )
    except Exception as e:
        print(f"❌ 錯誤: {e}", file=sys.stderr)
        if not args.dry_run:
            print(json.dumps({"success": False, "error": str(e)}, ensure_ascii=False))
        sys.exit(1)


if __name__ == "__main__":
    main()
