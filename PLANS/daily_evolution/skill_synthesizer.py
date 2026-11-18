#!/usr/bin/env python3
"""
Buzz 技能合成與經驗提煉引擎 (skill_synthesizer.py)

職責：
1. 全景回顧 WORK_LOGS/、Obsidian 核心記憶、冷記憶、對話精華、前沿筆記 (04-Social-Scraps) 與 inbox/
2. 雙軌能力識別：
   - 軌道 A (new)：從對話、記憶與痛點中辨識尚未技能化的高頻工作流，產出全新原生技能
   - 軌道 B (synthesis)：掃描現有已安裝技能庫，分析互補性並將多個現有技能「互相合成」為高階組合技能
3. 接收 Honey2 LLM 動態研究建議，實體草擬合規候選技能至 PLANS/SKILL_CANDIDATES/<name>/SKILL.md
4. 嚴格遵循隔離原則：僅生成候選技能，絕不私自安裝生效，須經主理人同意與 Sentry 審核

遵循標準：
- 語言：繁體中文 (Traditional Chinese)
- 技能規格：Buzz 原生 .agents/skills/<name>/SKILL.md 規範 (含 YAML Frontmatter)
- 程式風格：PEP 8、強型別標註、低複雜度、無魔術數字
"""

import json
import re
from pathlib import Path
from typing import Any, Dict, List, Optional

# 命名正則規範 (kebab-case)
KEBAB_CASE_PATTERN = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")

# 最小觸發詞頻門檻，避免單次偶然操作被誤升級為技能
MIN_OCCURRENCE_THRESHOLD = 2

# 預設掃描各類來源的最大篇數上限（避免上下文膨脹）
MAX_COLD_MEMORY_FILES = 6
MAX_DIGEST_FILES = 6
MAX_SCRAP_FILES = 8
MAX_INBOX_FILES = 6
MAX_WORKLOG_JSONL_LINES = 150


class SkillCandidate:
    """技能候選資料模型（支援新技能產出與既有技能互相合成）"""

    def __init__(
        self,
        name: str,
        title: str,
        description: str,
        trigger_conditions: List[str],
        prerequisites: List[str],
        steps: List[str],
        constraints: List[str],
        source_summary: str,
        kind: str = "new",
        base_skills: Optional[List[str]] = None,
        synthesis_reason: str = "",
    ):
        self.name = name
        self.title = title
        self.description = description.strip()
        self.trigger_conditions = trigger_conditions
        self.prerequisites = prerequisites
        self.steps = steps
        self.constraints = constraints
        self.source_summary = source_summary
        self.kind = kind  # "new" 或 "synthesis"
        self.base_skills = base_skills or []
        self.synthesis_reason = synthesis_reason or source_summary

    def to_markdown(self) -> str:
        """產出符合 Buzz 規範之 SKILL.md 內容 (含 YAML Frontmatter)"""
        triggers_md = "\n".join(f"- {cond}" for cond in self.trigger_conditions)
        prereqs_md = "\n".join(f"- {req}" for req in self.prerequisites)
        steps_md = "\n".join(f"{i}. {step}" for i, step in enumerate(self.steps, 1))
        constraints_md = "\n".join(f"- {c}" for c in self.constraints)

        kind_label = "既有技能互相合成 (Skill Synthesis)" if self.kind == "synthesis" else "新流程技能產出 (New Skill)"
        base_line = (
            f"> **合成配方**：`{' + '.join(self.base_skills)}` ➔ `{self.name}`  \n"
            if self.base_skills
            else ""
        )

        return f"""---
name: {self.name}
description: >
  {self.description}
version: 1
---

# {self.title}

> **類型**：{kind_label}  
{base_line}> **提煉來源**：{self.source_summary}  
> **狀態**：候選中 (Pending Approval)  
> **規格標準**：Buzz Native Skill v1.0

---

## 🎯 觸發場景 (Trigger Conditions)
{triggers_md}

---

## 📋 輸入與前置需求 (Prerequisites)
{prereqs_md}

---

## 🛠️ 標準作業程序 (Standard Operating Procedure)
{steps_md}

---

## 🔒 安全與授權約束 (Security & Authorization Constraints)
{constraints_md}
- 嚴格遵循 Buzz 代理人通訊紀律與老闆審批關卡 (Boss Approval Gatekeeper)
- 禁止私自執行未授權的高風險系統命令
"""


class SkillSynthesizer:
    """全景知識驅動的技能產出與互相合成器"""

    def __init__(
        self,
        workspace_root: Optional[str] = None,
        obsidian_vault_path: Optional[str] = None,
    ):
        if workspace_root:
            self.workspace_root = Path(workspace_root)
        else:
            self.workspace_root = Path(__file__).resolve().parent.parent.parent

        self.work_logs_dir = self.workspace_root / "WORK_LOGS"
        self.plans_dir = self.workspace_root / "PLANS"
        self.candidates_dir = self.plans_dir / "SKILL_CANDIDATES"
        self.user_buzz_work_logs = Path(r"C:\Users\user\.buzz\WORK_LOGS")

        if obsidian_vault_path:
            self.obsidian_root = Path(obsidian_vault_path)
            self.obsidian_digest_dir = self.obsidian_root / "wiki" / "02-Conversation-Digest"
        else:
            self.obsidian_root = Path(r"C:\私人\ai agent\buzz-obsidian")
            wiki_digest = self.obsidian_root / "wiki" / "02-Conversation-Digest"
            legacy_digest = self.obsidian_root / "02-Conversation-Digest"
            self.obsidian_digest_dir = wiki_digest if wiki_digest.exists() else legacy_digest

        self.candidates_dir.mkdir(parents=True, exist_ok=True)

    def _read_recent_markdown_dir(
        self, directory: Path, tag_prefix: str, limit: int, max_chars: int = 2500
    ) -> List[Dict[str, str]]:
        """安全讀取指定目錄下最新 N 份 Markdown 筆記"""
        items: List[Dict[str, str]] = []
        if not directory.exists():
            return items
        files = sorted(directory.rglob("*.md"), key=lambda p: p.stat().st_mtime, reverse=True)
        for p in files[:limit]:
            try:
                text = p.read_text(encoding="utf-8", errors="ignore").strip()
                if text:
                    items.append({"source": f"{tag_prefix}/{p.name}", "text": text[:max_chars]})
            except Exception:
                continue
        return items

    def collect_source_texts(self, include_memories_and_notes: bool = True) -> List[Dict[str, str]]:
        """蒐集對話日誌、核心記憶、冷記憶、前沿筆記與收件匣靈感"""
        sources: List[Dict[str, str]] = []

        # 1. 工作區 WORK_LOGS/
        if self.work_logs_dir.exists():
            for p in sorted(self.work_logs_dir.glob("*.md"), reverse=True)[:5]:
                try:
                    sources.append({"source": f"WORK_LOGS/{p.name}", "text": p.read_text(encoding="utf-8", errors="ignore")[:3000]})
                except Exception:
                    pass

        # 2. 使用者全域 JSONL 對話收割
        if self.user_buzz_work_logs.exists():
            for p in sorted(self.user_buzz_work_logs.glob("*.jsonl"), reverse=True)[:4]:
                try:
                    lines = p.read_text(encoding="utf-8", errors="ignore").splitlines()
                    combined_texts = []
                    for line in lines[-MAX_WORKLOG_JSONL_LINES:]:
                        try:
                            item = json.loads(line)
                            content = item.get("content", "")
                            if content and len(content) > 10:
                                combined_texts.append(content[:400])
                        except Exception:
                            continue
                    if combined_texts:
                        sources.append({"source": f"UserBuzz/{p.name}", "text": "\n".join(combined_texts)})
                except Exception:
                    pass

        # 3. Obsidian 記憶庫與筆記全景回顧
        if include_memories_and_notes:
            wiki_dir = self.obsidian_root / "wiki"
            core_dir = wiki_dir / "00-Core-Memory"
            if core_dir.exists():
                sources.extend(self._read_recent_markdown_dir(core_dir, "CoreMemory", 2, 2000))

            cold_dir = wiki_dir / "01-Cold-Memory"
            if cold_dir.exists():
                sources.extend(self._read_recent_markdown_dir(cold_dir, "ColdMemory", MAX_COLD_MEMORY_FILES, 2000))

            if self.obsidian_digest_dir.exists():
                sources.extend(self._read_recent_markdown_dir(self.obsidian_digest_dir, "Digest", MAX_DIGEST_FILES, 2500))

            scraps_dir = wiki_dir / "04-Social-Scraps"
            if scraps_dir.exists():
                sources.extend(self._read_recent_markdown_dir(scraps_dir, "SocialScraps", MAX_SCRAP_FILES, 2500))

            inbox_dir = self.obsidian_root / "inbox"
            if inbox_dir.exists():
                sources.extend(self._read_recent_markdown_dir(inbox_dir, "Inbox", MAX_INBOX_FILES, 2000))

        return sources

    def scan_installed_skills_detail(self) -> Dict[str, Dict[str, str]]:
        """掃描目前系統與專案中所有已安裝生效的技能名稱與簡介"""
        details: Dict[str, Dict[str, str]] = {}
        skill_dirs = [
            Path(r"C:\Users\user\.buzz\skills"),
            Path(r"C:\Users\user\.buzz\.agents\skills"),
            Path(r"C:\私人\skills"),
            Path(r"C:\私人\ai agent\buzz\skills"),
            Path(r"C:\私人\ai agent\buzz\.agents\skills"),
        ]
        for sdir in skill_dirs:
            if not sdir.exists():
                continue
            try:
                for item in sdir.iterdir():
                    if not item.is_dir() or item.name.startswith(".") or item.name == "_TEMPLATE":
                        continue
                    skill_key = item.name.lower().strip().replace("_", "-")
                    desc = ""
                    skill_md = item / "SKILL.md"
                    manifest = item / "manifest.json"
                    if skill_md.exists():
                        raw = skill_md.read_text(encoding="utf-8", errors="ignore")
                        m = re.search(r"description:\s*>?\s*\n?\s*([^\n]+)", raw)
                        desc = m.group(1).strip().strip('"') if m else raw[:120].replace("\n", " ")
                    elif manifest.exists():
                        try:
                            data = json.loads(manifest.read_text(encoding="utf-8", errors="ignore"))
                            desc = str(data.get("description", ""))
                        except Exception:
                            pass
                    elif (item / "Cargo.toml").exists():
                        raw = (item / "Cargo.toml").read_text(encoding="utf-8", errors="ignore")
                        m = re.search(r'description\s*=\s*"([^"]+)"', raw)
                        desc = m.group(1) if m else ""
                    if skill_key not in details or (not details[skill_key]["description"] and desc):
                        details[skill_key] = {"name": item.name, "description": desc, "path": str(item)}
            except Exception:
                continue
        return details

    def get_installed_skills(self) -> set:
        """獲取目前系統中已安裝生效之技能名稱集合"""
        return set(self.scan_installed_skills_detail().keys())

    def analyze_patterns(self, sources: List[Dict[str, str]]) -> List[SkillCandidate]:
        """
        分析來源文字中的高頻流程與架構痛點，合成出新技能候選。
        """
        combined_text = "\n".join(s["text"] for s in sources).lower()
        candidates: List[SkillCandidate] = []

        # 模式 1: Obsidian 知識庫與對話沉澱工作流
        obsidian_mentions = combined_text.count("obsidian") + combined_text.count("digest") + combined_text.count("知識庫")
        if obsidian_mentions >= MIN_OCCURRENCE_THRESHOLD:
            candidates.append(
                SkillCandidate(
                    name="obsidian-digest-sync",
                    title="Obsidian 對話摘要與第二大腦同步技能",
                    description="自動化對話摘要提取、標籤分類與 Obsidian 第二大腦雙向知識庫同步流程。",
                    trigger_conditions=[
                        "日常對話結束或定時收割時需要沉澱知識",
                        "使用者指示將重大決策或技術架構整理至 Obsidian",
                    ],
                    prerequisites=[
                        "本機已啟動 sync_obsidian/main.mjs 背景同步守護程式",
                        "知識庫目錄 `C:\\私人\\ai agent\\buzz-obsidian` 具備讀寫權限",
                    ],
                    steps=[
                        "檢查今日對話紀錄並過濾純機器狀態報告雜訊",
                        "依據時間戳與主題產出結構化摘要並更新 INDEX.md 索引表",
                        "執行機敏詞脫敏過濾並向頻道回報沉澱筆記路徑",
                    ],
                    constraints=["不得覆蓋已存在之歷史封存筆記", "禁止將私鑰寫入公開筆記"],
                    source_summary=f"對話紀錄與知識庫摘要高頻提煉 (詞頻: {obsidian_mentions})",
                    kind="new",
                )
            )

        # 模式 2: Git 雙軌升級與無損合流 SOP
        git_mentions = combined_text.count("upstream") + combined_text.count("rebase") + combined_text.count("無損升級")
        if git_mentions >= MIN_OCCURRENCE_THRESHOLD:
            candidates.append(
                SkillCandidate(
                    name="upstream-sync-resolver",
                    title="Buzz 官方雙軌無損同步與版本升級技能",
                    description="標準化官方 `upstream/main` 追蹤、本地 `main` 快進更新與 `feature/custom-local-core` 無損合流檢查流程。",
                    trigger_conditions=["官方發布新版本提交需要同步合流時"],
                    prerequisites=["Git 工作區乾淨", "已設定 `upstream` 遠端追蹤"],
                    steps=[
                        "執行 `git fetch upstream` 並檢視與本地修改檔案是否重疊",
                        "將本地與 origin 的 `main` 快進至 `upstream/main`",
                        "切回 `feature/custom-local-core` 執行無損 merge/rebase 並跑編譯測試",
                    ],
                    constraints=["嚴禁使用 `git push --force`", "嚴禁改動凍結檢查點 Tag"],
                    source_summary=f"雙軌版控無損合流歷程提煉 (詞頻: {git_mentions})",
                    kind="new",
                )
            )

        # 模式 3: 系統健康體檢與熔斷自癒 SOP
        health_mentions = combined_text.count("health") + combined_text.count("健康") + combined_text.count("熔斷")
        if health_mentions >= MIN_OCCURRENCE_THRESHOLD:
            candidates.append(
                SkillCandidate(
                    name="system-health-guardian",
                    title="Buzz 系統健康監控與熔斷自癒技能",
                    description="自動化執行 CLI 狀態診斷、環境變數檢驗與常駐守護重啟標準作業流程。",
                    trigger_conditions=["日常進化流程啟動前之例行體檢或守護行程異常時"],
                    prerequisites=["具備 Python 3 與 Node.js 執行環境"],
                    steps=["檢查常駐進程與端口健康度", "排查錯誤日誌並上報異常摘要"],
                    constraints=["遇不可逆崩潰必須立即停止 (STOP)"],
                    source_summary=f"系統維運歷史提煉 (詞頻: {health_mentions})",
                    kind="new",
                )
            )

        # 模式 4: 主動排程與事件觸發編排 (Proactive Autonomous Triggers)
        trigger_mentions = (
            combined_text.count("proactive")
            + combined_text.count("trigger")
            + combined_text.count("always-on")
            + combined_text.count("cron")
        )
        if trigger_mentions >= MIN_OCCURRENCE_THRESHOLD:
            candidates.append(
                SkillCandidate(
                    name="proactive-trigger-orchestrator",
                    title="主動式事件與 Cron 觸發器編排技能",
                    description="指導特工安全註冊 Cron 排程、EventBus 事件訂閱與優雅關閉流程，避免背景孤兒進程殘留。",
                    trigger_conditions=[
                        "特工需要建立定時巡檢、檔案變更監聽或事件驅動主動通報任務時",
                        "需要串接 `RunCtx` 的 `EventBus` 與 `Scheduler` 觸發回呼時",
                    ],
                    prerequisites=[
                        "核心引擎已實裝 `crates/buzz-agent/src/triggers/` (已通過 19 項測試)",
                        "任務處理邏輯具備冪等性與取消安全 (CancellationToken)",
                    ],
                    steps=[
                        "使用 `preview_cron` / `parse_interval` 驗證排程週期並加入合理 Jitter 抖動",
                        "透過 `TriggerRegistry` 註冊觸發器與對應事件類型，設定背壓上限",
                        "實作明確的 Shutdown hook 確保關閉時持久化狀態並回收 Tokio 任務",
                    ],
                    constraints=[
                        "嚴禁建立高頻輪詢 (< 30s) 造成 API 429 耗盡",
                        "所有定時發送給主理人的通知優先遵守 DM 私訊規定",
                    ],
                    source_summary=f"Proactive Triggers 實作與 Always-On 筆記提煉 (觸發詞頻: {trigger_mentions})",
                    kind="new",
                    synthesis_reason="結合 Proactive Triggers 模組與 Always-On 筆記，提供特工自主安全排程規範",
                )
            )

        # 模式 5: 長程任務 7200s 防超時分段交付護欄
        long_turn_mentions = (
            combined_text.count("7200")
            + combined_text.count("timeout")
            + combined_text.count("續航")
            + combined_text.count("checkpoint")
            + combined_text.count("超時")
        )
        if long_turn_mentions >= MIN_OCCURRENCE_THRESHOLD:
            candidates.append(
                SkillCandidate(
                    name="long-turn-checkpoint-guard",
                    title="長程開發任務斷點存檔與分段結案護欄技能",
                    description="防止特工在單一大型任務中連續運行超過 `buzz-acp` 的 7200 秒硬性天花板，透過階段性檢查點強制階段結案與無縫接關。",
                    trigger_conditions=[
                        "任務預估超過 80 輪工具調用或涵蓋跨模組重構 + 整合測試時",
                        "任務已連續執行超過 50 分鐘時自動切分為子里程碑",
                    ],
                    prerequisites=[
                        "專案具備 `git status` 與增量編譯驗證能力",
                        "對話串支援接力回覆機制",
                    ],
                    steps=[
                        "開工前將巨型工單拆分為獨立可驗收的里程碑 (Milestone 1/2/3)",
                        "每完成一個可編譯模組並通過單元測試，立即落盤存檔並提交階段報告",
                        "於單回合耗時達 60 分鐘前主動結束本回合，建議主理人發起下一接續回合避開 7200s 強制切斷",
                    ],
                    constraints=[
                        "斷點交接時必須在討論串留下可複製的接關上下文與測試結果",
                        "確保磁碟上絕不留下一半未閉合語法錯誤的中間狀態",
                    ],
                    source_summary=f"由夜間 600 輪 / 7200s 超時復盤與長程日誌提煉 (觸發詞頻: {long_turn_mentions})",
                    kind="new",
                    synthesis_reason="根治大型任務撞上 `buzz-acp` 7200 秒單回合超時門檻的實戰護欄",
                )
            )

        return candidates

    def synthesize_skill_compositions(
        self, installed_details: Dict[str, Dict[str, str]], sources: List[Dict[str, str]]
    ) -> List[SkillCandidate]:
        """
        分析現有已安裝技能之間的互補性，結合記憶與筆記需求，提出『技能互相合成』候選方案。
        """
        installed_keys = set(installed_details.keys())
        combined_text = "\n".join(s["text"] for s in sources).lower()
        synthesized: List[SkillCandidate] = []

        # 合成配方 1: browser-automation + (obsidian-digest-sync | daily-digest) -> web-intel-to-obsidian
        if "browser-automation" in installed_keys and (
            "obsidian-digest-sync" in installed_keys or "daily-digest" in installed_keys
        ):
            synthesized.append(
                SkillCandidate(
                    name="web-intel-to-obsidian",
                    title="網頁情報自動擷取與第二大腦沉澱合成技能",
                    description="將 `browser-automation` 的動態網頁渲染抓取與 `obsidian-digest-sync` 的結構化整理合成為單一管線，自動將 GitHub 專案/技術文章轉存為 Obsidian 雙鏈筆記。",
                    trigger_conditions=[
                        "主理人在討論串或 `inbox/` 丟入 URL 並要求閱讀與整理時",
                        "夜間 GitHub 雷達需要對動態 SPA 頁面進行深度擷取並沉澱進筆記時",
                    ],
                    prerequisites=[
                        "已啟用 `browser-automation` 安全網域白名單與 Playwright 環境",
                        "已啟用 `obsidian-digest-sync` 寫入 `04-Social-Scraps/`",
                    ],
                    steps=[
                        "呼叫 `browser-automation` 安全造訪目標網頁並擷取乾淨正文與結構化欄位",
                        "使用 Honey2 語義提煉出「核心用途、架構亮點、與 Buzz 的結合點」",
                        "呼叫 `obsidian-digest-sync` 自動建立 `YYYY-MM-DD_<主題>.md` 並更新 `INDEX.md` 雙鏈",
                        "完成後回傳 Obsidian 筆記連結並於明日進化報告引用",
                    ],
                    constraints=[
                        "嚴格遵守 SSRF 網域與私人 IP 黑名單攔截規範",
                        "寫入筆記前必須執行金鑰與 PII 脫敏過濾",
                    ],
                    source_summary="跨技能合成：browser-automation + obsidian-digest-sync（配合 Social-Scraps / inbox 筆記累積）",
                    kind="synthesis",
                    base_skills=["browser-automation", "obsidian-digest-sync"],
                    synthesis_reason="結合瀏覽器動態抓取與第二大腦自動入庫，免去人工複製網頁進 inbox 的重複勞動",
                )
            )

        # 合成配方 2: alpha-research + trend-following + var-models -> quant-portfolio-risk-sentinel
        if {"alpha-research", "trend-following", "var-models"}.issubset(installed_keys):
            synthesized.append(
                SkillCandidate(
                    name="quant-portfolio-risk-sentinel",
                    title="量化持倉趨勢與 VaR 風險自動化巡檢合成技能",
                    description="合成 `alpha-research`、`trend-following` 與 `var-models` 三大量化技能，對關注標的執行趨勢突破訊號分析與尾部風險 (VaR/CVaR) 定時健檢。",
                    trigger_conditions=[
                        "主理人詢問持有標的（如港股/ETF 組合）趨勢動量與風險回撤評估時",
                        "透過 Proactive Triggers 進行每週投資組合風險覆盤時",
                    ],
                    prerequisites=[
                        "在庫已具備 `alpha-research`、`trend-following` 與 `var-models`",
                        "具備公開市場歷史價格數據存取管道",
                    ],
                    steps=[
                        "以 `trend-following` 計算關注標的多週期動量趨勢與 ATR 波動率水位",
                        "以 `alpha-research` 檢驗 Alpha 因子衰減與多信號強弱對比",
                        "呼叫 `var-models` 計算 95%/99% 信賴水準下的 VaR 與壓力情境虧損",
                        "輸出結構化風險儀表板報告並透過 DM 私訊推播給主理人",
                    ],
                    constraints=[
                        "僅供輔助決策參考，嚴禁自動執行任何真實下單或資金異動指令",
                        "明確揭露數據來源與計算假設",
                    ],
                    source_summary="跨技能合成：alpha-research + trend-following + var-models",
                    kind="synthesis",
                    base_skills=["alpha-research", "trend-following", "var-models"],
                    synthesis_reason="整合在庫三套孤立的量化工具技能，轉化為主理人長期持股組合的自動化風險健檢哨兵",
                )
            )

        # 合成配方 3: qa-guard + env-guardian + system-health-guardian -> zero-defect-release-gate
        if {"qa-guard", "env-guardian", "system-health-guardian"}.issubset(installed_keys):
            synthesized.append(
                SkillCandidate(
                    name="zero-defect-release-gate",
                    title="熱部署前環境、編碼與語法全自動三合一門禁合成技能",
                    description="合成 `qa-guard`、`env-guardian` 與 `system-health-guardian`，在發布新版 `buzz-agent.exe` 或提交代碼前執行環境完整度、UTF-8 無 BOM+LF、`cargo check` 0 warning 與守護進程健康度一站式檢查。",
                    trigger_conditions=[
                        "Pollen 完成代碼開發準備請求 Sentry 驗收時",
                        "準備執行 `cargo build --release` 與熱替換本地執行檔前",
                    ],
                    prerequisites=[
                        "在庫已具備 `qa-guard`、`env-guardian`、`system-health-guardian`",
                    ],
                    steps=[
                        "執行 `env-guardian` 確認必要環境變數、工具鏈與本地端口 (3000/8888) 正常",
                        "執行 `qa-guard` 檢查 Git Patch UTF-8 無 BOM、LF 換行符與靜態語法完整性",
                        "驗證所有目標 crate 之 `cargo check --all-targets` 達到 0 Error / 0 Warning",
                        "於部署後調用 `system-health-guardian` 確認所有背景特工重連成功且無孤兒進程",
                    ],
                    constraints=[
                        "任一門禁未通過即阻斷發版，嚴禁帶錯強制覆蓋線上執行檔",
                    ],
                    source_summary="跨技能合成：qa-guard + env-guardian + system-health-guardian",
                    kind="synthesis",
                    base_skills=["qa-guard", "env-guardian", "system-health-guardian"],
                    synthesis_reason="將三個品質檢測技能串成發版流水線，從源頭杜絕 IDE Problems 累積與環境缺漏",
                )
            )

        # 若對話或筆記出現特定需求，優先推薦關聯度最高的合成配方
        if "portfolio" in combined_text or "持股" in combined_text or "紅利etf" in combined_text:
            synthesized.sort(key=lambda c: 0 if c.name == "quant-portfolio-risk-sentinel" else 1)

        return synthesized

    def build_candidates_from_llm_ideas(
        self, llm_ideas: Optional[List[Dict[str, Any]]], installed_keys: set
    ) -> List[SkillCandidate]:
        """將 Honey2 大模型動態研究的技能產出／合成創意安全轉換為候選檔案"""
        if not llm_ideas:
            return []
        dynamic_candidates: List[SkillCandidate] = []
        for idea in llm_ideas:
            raw_name = str(idea.get("name", "")).strip().lower().replace("_", "-")
            if not raw_name or not KEBAB_CASE_PATTERN.match(raw_name):
                continue
            if raw_name in installed_keys:
                continue
            title = str(idea.get("title", raw_name)).strip()
            desc = str(idea.get("description", "")).strip()
            reason = str(idea.get("reason", "Honey2 深度回顧對話與記憶提煉")).strip()
            base_skills = [str(s).strip() for s in idea.get("base_skills", []) if str(s).strip()]
            kind = "synthesis" if (idea.get("kind") == "synthesis" or len(base_skills) >= 2) else "new"
            steps = [str(s).strip() for s in idea.get("steps", []) if str(s).strip()]
            if not desc or not steps:
                continue
            dynamic_candidates.append(
                SkillCandidate(
                    name=raw_name,
                    title=title,
                    description=desc,
                    trigger_conditions=[str(t).strip() for t in idea.get("triggers", []) if str(t).strip()]
                    or ["依每日進化提案審查批准後觸發使用"],
                    prerequisites=["遵守 Buzz 安全門禁與本機工作目錄規範"],
                    steps=steps,
                    constraints=["須經主理人同意並通過 Sentry 四大語意審核後掛載"],
                    source_summary=reason,
                    kind=kind,
                    base_skills=base_skills,
                    synthesis_reason=reason,
                )
            )
        return dynamic_candidates

    def save_candidates(self, candidates: List[SkillCandidate]) -> List[Path]:
        """將技能候選存檔至 PLANS/SKILL_CANDIDATES/<name>/SKILL.md"""
        saved_paths: List[Path] = []
        for cand in candidates:
            cand_dir = self.candidates_dir / cand.name
            cand_dir.mkdir(parents=True, exist_ok=True)
            skill_file = cand_dir / "SKILL.md"
            skill_file.write_text(cand.to_markdown(), encoding="utf-8")
            saved_paths.append(skill_file)
        return saved_paths

    def detect_and_synthesize(
        self,
        extra_texts: Optional[List[Dict[str, str]]] = None,
        llm_skill_ideas: Optional[List[Dict[str, Any]]] = None,
        include_memories_and_notes: bool = True,
    ) -> List[Dict[str, Any]]:
        """執行全景模式分析、跨技能互相合成與候選存檔，自動排除已在庫之技能"""
        sources = self.collect_source_texts(include_memories_and_notes=include_memories_and_notes)
        if extra_texts:
            sources.extend(extra_texts)

        installed_details = self.scan_installed_skills_detail()
        installed = set(installed_details.keys())

        new_candidates = self.analyze_patterns(sources)
        synth_candidates = self.synthesize_skill_compositions(installed_details, sources)
        llm_candidates = self.build_candidates_from_llm_ideas(llm_skill_ideas, installed)

        # 去重合併：優先保留 Honey2 動態建議，其次合成與規則提煉；已在庫技能排除
        seen_names = set()
        merged: List[SkillCandidate] = []
        for cand in llm_candidates + new_candidates + synth_candidates:
            key = cand.name.lower().strip()
            if key in installed or key in seen_names:
                continue
            seen_names.add(key)
            merged.append(cand)

        saved_paths = self.save_candidates(merged)

        results: List[Dict[str, Any]] = []
        for cand, path in zip(merged, saved_paths):
            results.append(
                {
                    "name": cand.name,
                    "title": cand.title,
                    "kind": cand.kind,
                    "base_skills": cand.base_skills,
                    "description": cand.description,
                    "synthesis_reason": cand.synthesis_reason,
                    "path": str(path),
                    "source_summary": cand.source_summary,
                }
            )
        return results


def main():
    synthesizer = SkillSynthesizer()
    results = synthesizer.detect_and_synthesize()
    print(f"✅ 技能產出與合成完成，共產出 {len(results)} 項候選：")
    for r in results:
        tag = "合成" if r["kind"] == "synthesis" else "新產出"
        print(f" - [{tag}] [{r['name']}] {r['title']} -> {r['path']}")


if __name__ == "__main__":
    main()
