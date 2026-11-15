#!/usr/bin/env python3
"""
Buzz 技能合成與經驗提煉引擎 (skill_synthesizer.py)

職責：
1. 分析 WORK_LOGS/ 與 Obsidian 每日知識庫對話摘要
2. 辨識高頻重複作業流程 (SOP) 與工具鏈組合
3. 自動草擬符合 Buzz 原生規格的候選技能 (PLANS/SKILL_CANDIDATES/<skill-name>/SKILL.md)
4. 嚴格遵循隔離原則：僅生成候選技能，絕不私自安裝生效

遵循標準：
- 語言：繁體中文 (Traditional Chinese)
- 技能規格：Buzz 原生 .agents/skills/<name>/SKILL.md 規範 (含 YAML Frontmatter)
- 程式風格：PEP 8、強型別標註、低複雜度、無魔術數字
"""

import json
import os
import re
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional

# 台北/香港時區 (UTC+8)
TZ_TAIPEI = timezone(timedelta(hours=8))

# 命名正則規範 (kebab-case)
KEBAB_CASE_PATTERN = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")

# 最小觸發詞頻門檻，避免單次偶然操作被誤升級為技能
MIN_OCCURRENCE_THRESHOLD = 2


class SkillCandidate:
    """技能候選資料模型"""

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
    ):
        self.name = name
        self.title = title
        self.description = description.strip()
        self.trigger_conditions = trigger_conditions
        self.prerequisites = prerequisites
        self.steps = steps
        self.constraints = constraints
        self.source_summary = source_summary

    def to_markdown(self) -> str:
        """產出符合 Buzz 規範之 SKILL.md 內容 (含 YAML Frontmatter)"""
        triggers_md = "\n".join(f"- {cond}" for cond in self.trigger_conditions)
        prereqs_md = "\n".join(f"- {req}" for req in self.prerequisites)
        steps_md = "\n".join(f"{i}. {step}" for i, step in enumerate(self.steps, 1))
        constraints_md = "\n".join(f"- {c}" for c in self.constraints)

        return f"""---
name: {self.name}
description: >
  {self.description}
version: 1
---

# {self.title}

> **提煉來源**：{self.source_summary}  
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
    """技能合成與提煉器"""

    def __init__(
        self,
        workspace_root: Optional[str] = None,
        obsidian_vault_path: Optional[str] = None,
    ):
        if workspace_root:
            self.workspace_root = Path(workspace_root)
        else:
            # 預設錨定為專案根目錄 (即 scripts/ 之上一層)
            self.workspace_root = Path(__file__).resolve().parent.parent.parent
        self.work_logs_dir = self.workspace_root / "WORK_LOGS"
        self.plans_dir = self.workspace_root / "PLANS"
        self.candidates_dir = self.plans_dir / "SKILL_CANDIDATES"

        # 使用者目錄下的工作日誌作為補充來源
        self.user_buzz_work_logs = Path("C:/Users/user/.buzz/WORK_LOGS")

        # Obsidian 知識庫對話摘要路徑
        if obsidian_vault_path:
            self.obsidian_digest_dir = Path(obsidian_vault_path)
        else:
            wiki_digest = Path("C:/私人/ai agent/buzz-obsidian/wiki/02-Conversation-Digest")
            legacy_digest = Path("C:/私人/ai agent/buzz-obsidian/02-Conversation-Digest")
            self.obsidian_digest_dir = wiki_digest if wiki_digest.exists() else legacy_digest

        # 確保候選輸出目錄存在
        self.candidates_dir.mkdir(parents=True, exist_ok=True)

    def collect_source_texts(self) -> List[Dict[str, str]]:
        """蒐集所有可用之經驗來源 (WORK_LOGS 與 Obsidian 知識庫摘要)"""
        sources: List[Dict[str, str]] = []

        # 1. 蒐集專案 WORK_LOGS/
        if self.work_logs_dir.exists():
            for p in self.work_logs_dir.glob("*.md"):
                try:
                    sources.append({"source": f"WORK_LOGS/{p.name}", "text": p.read_text(encoding="utf-8")})
                except Exception:
                    pass

        # 2. 蒐集使用者全域 WORK_LOGS/
        if self.user_buzz_work_logs.exists():
            for p in self.user_buzz_work_logs.glob("*.jsonl"):
                try:
                    # 擷取 JSONL 中的訊息文字
                    lines = p.read_text(encoding="utf-8").splitlines()
                    combined_texts = []
                    for line in lines[-100:]:  # 讀取最新 100 筆
                        try:
                            item = json.loads(line)
                            content = item.get("content", "")
                            if content and len(content) > 10:
                                combined_texts.append(content)
                        except Exception:
                            continue
                    if combined_texts:
                        sources.append({"source": f"UserBuzz/{p.name}", "text": "\n".join(combined_texts)})
                except Exception:
                    pass

        # 3. 蒐集 Obsidian 對話摘要 (最新 5 篇)
        if self.obsidian_digest_dir.exists():
            digest_files = sorted(self.obsidian_digest_dir.glob("*.md"), reverse=True)
            for p in digest_files[:5]:
                try:
                    sources.append({"source": f"Obsidian/{p.name}", "text": p.read_text(encoding="utf-8")})
                except Exception:
                    pass

        return sources

    def analyze_patterns(self, sources: List[Dict[str, str]]) -> List[SkillCandidate]:
        """
        分析來源文字中的高頻模式，並合成出具體技能候選。
        
        模式辨識策略：
        1. 知識庫與 Obsidian 雙向同步維護 SOP
        2. Git 升級演進與無損合併審核 SOP
        3. 系統健康體檢與熔斷自癒排查 SOP
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
                        "需要維護 02-Conversation-Digest 目錄之 Markdown 結構時",
                    ],
                    prerequisites=[
                        "本機已安裝並啟動 sync_obsidian/main.mjs 背景同步守護程式",
                        "知識庫目錄 C:/私人/ai agent/buzz-obsidian 具備讀寫權限",
                        "內容必須使用 UTF-8 (no BOM) 編碼與繁體中文",
                    ],
                    steps=[
                        "檢查今日對話紀錄並過濾純機器狀態報告雜訊",
                        "依據時間戳與主題進行歸納，產生符合 YYYY-MM-DD_主題.md 之摘要檔",
                        "自動添加 Frontmatter 標籤 (tags: [digest, buzz-memory])",
                        "透過 sync_obsidian 寫入 Obsidian Vault 並更新 INDEX.md 索引表",
                        "向頻道回報沉澱筆記之相對路徑與核心提要",
                    ],
                    constraints=[
                        "不得覆蓋已存在之歷史封存筆記",
                        "禁止將敏感金鑰或 Token 寫入公開筆記中",
                    ],
                    source_summary=f"自近期 {len(sources)} 份對話紀錄與知識庫摘要中高頻提煉 (觸發詞頻: {obsidian_mentions})",
                )
            )

        # 模式 2: Git 升級演進與無損代碼合流 SOP
        git_mentions = combined_text.count("upstream") + combined_text.count("merge") + combined_text.count("升級")
        if git_mentions >= MIN_OCCURRENCE_THRESHOLD:
            candidates.append(
                SkillCandidate(
                    name="upstream-sync-resolver",
                    title="Buzz 上游無損追蹤與分支升級技能",
                    description="標準化官方 upstream-track 分支無損追蹤、合流衝突排查與健康驗證作業流程。",
                    trigger_conditions=[
                        "官方發布新版本或新 commit 需要整合時",
                        "本地自訂功能需與 upstream-track 進行無衝突合併時",
                        "執行版本升級前後的自動化迴歸體檢",
                    ],
                    prerequisites=[
                        "Git 工作區必須為 Clean 狀態 (無未提交之修改)",
                        "已配置 upstream-track 遠端追蹤分支",
                        "本地執行環境可調用 health_check.py 進行驗收",
                    ],
                    steps=[
                        "執行 git status 確認工作目錄乾淨無未存變更",
                        "切換至 upstream-track 執行 git fetch 抓取官方最新變更",
                        "切換回 main 分支並發起 git merge upstream-track 合流",
                        "若遇衝突，依據核心業務優先原則進行排查修復",
                        "執行 python health_check.py 確保升級後系統狀態維持 HEALTHY",
                    ],
                    constraints=[
                        "嚴禁使用 git push --force 破壞歷史紀錄",
                        "合流修改必須經過 Sentry 審查與老闆最終確認",
                    ],
                    source_summary=f"自官方版本追蹤升級之實作歷史中提煉 (觸發詞頻: {git_mentions})",
                )
            )

        # 模式 3: 系統健康體檢與熔斷自癒排查 SOP
        health_mentions = combined_text.count("health") + combined_text.count("健康") + combined_text.count("熔斷")
        if health_mentions >= MIN_OCCURRENCE_THRESHOLD:
            candidates.append(
                SkillCandidate(
                    name="system-health-guardian",
                    title="Buzz 系統健康監控與熔斷自癒技能",
                    description="自動化執行 CLI 狀態診斷、環境變數檢驗、熔斷日誌排查與常駐守護重啟標準作業流程。",
                    trigger_conditions=[
                        "日常進化流程啟動前之例行前置體檢",
                        "背景守護行程異常退出或通訊失敗時",
                        "熔斷器跳脫 (Circuit Breaker Tripped) 時之故障排查",
                    ],
                    prerequisites=[
                        "本機具備 Python 3 與 Node.js 執行環境",
                        "具備讀取 WORK_LOGS/daily_evolution_failure.log 權限",
                        "可調用 buzz-background-services.vbs 進行常駐守護管理",
                    ],
                    steps=[
                        "執行 health_check.py 檢查 Buzz CLI 與核心變數",
                        "檢查 WORK_LOGS/ 下之失敗日誌與錯誤堆疊",
                        "檢視 3 大背景常駐行程 (OpenRouter, Sync, Proxy) 運作狀態",
                        "若發現熔斷日誌，排查根本原因並於確認修復後進行重置",
                        "彙整健全性報告呈報至專屬頻道",
                    ],
                    constraints=[
                        "遇不可逆之系統崩潰必須立即停止 (STOP) 並通知老闆",
                        "禁止私自清除未排查之關鍵錯誤日誌",
                    ],
                    source_summary=f"自系統維運歷史與健康檢查機制中提煉 (觸發詞頻: {health_mentions})",
                )
            )

        return candidates

    def get_installed_skills(self) -> set:
        """獲取目前系統中已安裝生效之技能名稱集合 (包含 .buzz/skills 與 C:/私人/skills)"""
        installed = set()
        skill_dirs = [
            Path("C:/Users/user/.buzz/skills"),
            Path("C:/私人/skills"),
        ]
        for sdir in skill_dirs:
            if sdir.exists():
                try:
                    for item in sdir.iterdir():
                        if item.is_dir() and item.name != "_TEMPLATE":
                            # 凡目錄存在且包含 SKILL.md、manifest.json 或 main.js 均視為在庫技能
                            if (
                                (item / "SKILL.md").exists()
                                or (item / "manifest.json").exists()
                                or (item / "main.js").exists()
                            ):
                                installed.add(item.name.lower().strip())
                except Exception:
                    pass
        return installed

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

    def detect_and_synthesize(self) -> List[Dict[str, Any]]:
        """執行完整的模式分析、技能合成與候選存檔流程，自動排除已在庫之技能"""
        sources = self.collect_source_texts()
        candidates = self.analyze_patterns(sources)
        installed = self.get_installed_skills()

        # 核心去重過濾：已在系統在庫之技能絕不重複推薦
        uninstalled_candidates = [
            c for c in candidates if c.name.lower().strip() not in installed
        ]

        saved_paths = self.save_candidates(uninstalled_candidates)

        results = []
        for cand, path in zip(uninstalled_candidates, saved_paths):
            results.append({
                "name": cand.name,
                "title": cand.title,
                "description": cand.description,
                "path": str(path),
                "source_summary": cand.source_summary,
            })
        return results


def main():
    synthesizer = SkillSynthesizer()
    results = synthesizer.detect_and_synthesize()
    print(f"✅ 技能合成完成，共產出 {len(results)} 項技能候選：")
    for r in results:
        print(f" - [{r['name']}] {r['title']} -> {r['path']}")


if __name__ == "__main__":
    main()
