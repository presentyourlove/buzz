#!/usr/bin/env python3
"""
Buzz 技能安全審查與一鍵落庫掛載腳本 (activate_skill.py)

職責：
1. 充當 Sentry 安全守門人：嚴格驗證技能命名、Frontmatter 規格、防範惡意提示詞注入
2. 原子化落庫 (Atomic Install)：
   - 同步寫入當前專案庫 .agents/skills/<skill-name>/SKILL.md
   - 同步寫入全域系統庫 C:/Users/user/.buzz/.agents/skills/<skill-name>/SKILL.md
3. 維護註冊表 (PLANS/SKILL_REGISTRY.json) 與演進履歷 (WORK_LOGS/SKILL_EVOLUTION.md)
4. 使 Buzz 內建 hints.rs 於下次載入時自動無感掛載全新技能

遵循標準：
- 語言：繁體中文 (Traditional Chinese)
- 安全原則：Sentry Gatekeeper，嚴格防範 Path Traversal 與 Injection
- 程式風格：PEP 8、強型別標註、無魔術數字
"""

import argparse
import json
import os
import re
import shutil
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# 台北/香港時區 (UTC+8)
TZ_TAIPEI = timezone(timedelta(hours=8))

# 命名規範：必須為合規 kebab-case
KEBAB_CASE_PATTERN = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")

# Sentry 禁用之高危特徵碼 (防止 Prompt Injection 與高危破壞命令)
PROHIBITED_PATTERNS = [
    re.compile(r"ignore\s+(all\s+)?(previous|prior)\s+instructions", re.IGNORECASE),
    re.compile(r"override\s+system\s+prompt", re.IGNORECASE),
    re.compile(r"bypass\s+(security|auth|approval)", re.IGNORECASE),
    re.compile(r"rm\s+-rf\s+[/~]", re.IGNORECASE),
    re.compile(r"format\s+[a-z]:", re.IGNORECASE),
    re.compile(r"del\s+/[fsq]\s+[a-z]:\\windows", re.IGNORECASE),
    re.compile(r"cat\s+.*id_rsa", re.IGNORECASE),
    re.compile(r"print.*BUZZ_PRIVATE_KEY", re.IGNORECASE),
]


class SentrySkillAuditor:
    """Sentry 技能安全審核器"""

    @staticmethod
    def audit_skill(skill_name: str, content: str) -> Tuple[bool, List[str]]:
        """
        執行多維度安全檢驗。
        回傳：(是否通過, 錯誤原因清單)
        """
        errors: List[str] = []

        # 1. 檢驗名稱命名規範
        if not KEBAB_CASE_PATTERN.match(skill_name):
            errors.append(f"技能名稱 '{skill_name}' 不符合 kebab-case 命名慣例 (僅允許小寫英文字母、數字與連字號)")

        # 2. 檢驗 YAML Frontmatter 邊界
        if not content.startswith("---"):
            errors.append("技能檔案缺少開頭 YAML Frontmatter 宣告 ('---')")

        parts = content.split("---", 2)
        if len(parts) < 3:
            errors.append("YAML Frontmatter 格式未閉合 (必須由前後各一對 '---' 包覆)")
            return False, errors

        frontmatter = parts[1]

        # 3. 檢驗 Frontmatter 必填欄位 (name, description)
        name_match = re.search(r"^name:\s*([^\s\n]+)", frontmatter, re.MULTILINE)
        if not name_match:
            errors.append("Frontmatter 缺少必填 'name' 欄位")
        elif name_match.group(1).strip() != skill_name:
            errors.append(f"Frontmatter 中宣告之 name ('{name_match.group(1).strip()}') 與資料夾名稱 ('{skill_name}') 不一致")

        if not re.search(r"^description:\s*", frontmatter, re.MULTILINE):
            errors.append("Frontmatter 缺少必填 'description' 欄位")

        # 4. 高危安全掃描 (防範 Prompt 越權攻擊與破壞指令)
        for pattern in PROHIBITED_PATTERNS:
            if pattern.search(content):
                errors.append(f"Sentry 攔截到潛在高危攻擊模式: '{pattern.pattern}'")

        return (len(errors) == 0), errors


class SkillActivator:
    """技能落庫與掛載器"""

    def __init__(
        self,
        workspace_root: Optional[str] = None,
        user_buzz_path: Optional[str] = None,
    ):
        if workspace_root:
            self.workspace_root = Path(workspace_root)
        else:
            self.workspace_root = Path(__file__).resolve().parent.parent.parent

        self.candidates_dir = self.workspace_root / "PLANS" / "SKILL_CANDIDATES"
        self.repo_skills_dir = self.workspace_root / ".agents" / "skills"
        self.plans_dir = self.workspace_root / "PLANS"
        self.work_logs_dir = self.workspace_root / "WORK_LOGS"
        self.registry_file = self.plans_dir / "SKILL_REGISTRY.json"
        self.evolution_log_file = self.work_logs_dir / "SKILL_EVOLUTION.md"

        if user_buzz_path:
            self.user_buzz_dir = Path(user_buzz_path)
        else:
            self.user_buzz_dir = Path("C:/Users/user/.buzz")
        self.user_skills_dir = self.user_buzz_dir / ".agents" / "skills"

    def activate(
        self,
        skill_name: str,
        approver: str = "可愛小寶寶 (老闆)",
        dry_run: bool = False,
    ) -> Dict[str, Any]:
        """執行 Sentry 審核、雙軌原子落庫與註冊登記"""
        candidate_file = self.candidates_dir / skill_name / "SKILL.md"

        # 1. 檢查候選檔案是否存在
        if not candidate_file.exists():
            raise FileNotFoundError(f"找不到技能候選檔案：{candidate_file}")

        content = candidate_file.read_text(encoding="utf-8")

        # 2. Sentry 安全總監把關
        is_safe, audit_errors = SentrySkillAuditor.audit_skill(skill_name, content)
        if not is_safe:
            raise PermissionError(f"Sentry 安全驗核未通過，拒絕落庫：{'; '.join(audit_errors)}")

        now = datetime.now(TZ_TAIPEI)
        timestamp_str = now.isoformat()

        if dry_run:
            return {
                "success": True,
                "dry_run": True,
                "skill_name": skill_name,
                "approver": approver,
                "installed_at": timestamp_str,
                "message": f"演練驗證通過：{skill_name} 具備完整合規性，可安全掛載。",
            }

        # 3. 雙軌原子落庫 (Repo 與 User .buzz)
        destinations: List[Path] = []

        # (a) 寫入專案庫
        repo_target_dir = self.repo_skills_dir / skill_name
        repo_target_dir.mkdir(parents=True, exist_ok=True)
        repo_target_file = repo_target_dir / "SKILL.md"
        repo_target_file.write_text(content, encoding="utf-8")
        destinations.append(repo_target_file)

        # (b) 寫入全域系統庫
        if self.user_buzz_dir.exists():
            user_target_dir = self.user_skills_dir / skill_name
            user_target_dir.mkdir(parents=True, exist_ok=True)
            user_target_file = user_target_dir / "SKILL.md"
            user_target_file.write_text(content, encoding="utf-8")
            destinations.append(user_target_file)

        # 4. 更新技能註冊表 (PLANS/SKILL_REGISTRY.json)
        registry_data: Dict[str, Any] = {"version": "1.0", "updated_at": timestamp_str, "skills": []}
        if self.registry_file.exists():
            try:
                registry_data = json.loads(self.registry_file.read_text(encoding="utf-8"))
            except Exception:
                pass

        # 移除已存在的舊記錄以避免重複
        existing_skills = [s for s in registry_data.get("skills", []) if s.get("name") != skill_name]
        existing_skills.append({
            "name": skill_name,
            "installed_at": timestamp_str,
            "approver": approver,
            "source_candidate": str(candidate_file.relative_to(self.workspace_root)),
            "active": True,
        })
        registry_data["skills"] = existing_skills
        registry_data["updated_at"] = timestamp_str

        self.registry_file.write_text(json.dumps(registry_data, indent=2, ensure_ascii=False), encoding="utf-8")

        # 同步註冊表至全域
        if self.user_buzz_dir.exists():
            user_plans = self.user_buzz_dir / "PLANS"
            user_plans.mkdir(parents=True, exist_ok=True)
            (user_plans / "SKILL_REGISTRY.json").write_text(json.dumps(registry_data, indent=2, ensure_ascii=False), encoding="utf-8")

        # 5. 寫入 WORK_LOGS/SKILL_EVOLUTION.md
        self.work_logs_dir.mkdir(parents=True, exist_ok=True)
        log_entry = f"- **[{now.strftime('%Y-%m-%d %H:%M:%S')}]** 技能 `{skill_name}` 經由 **{approver}** 批准，已正式落庫掛載至全體 Agent。\n"
        if not self.evolution_log_file.exists():
            self.evolution_log_file.write_text("# 🛠️ Buzz 技能自主進化履歷表 (Skill Evolution Log)\n\n", encoding="utf-8")
        with open(self.evolution_log_file, "a", encoding="utf-8") as f:
            f.write(log_entry)

        return {
            "success": True,
            "dry_run": False,
            "skill_name": skill_name,
            "approver": approver,
            "installed_at": timestamp_str,
            "destinations": [str(p) for p in destinations],
            "message": f"🎉 技能 '{skill_name}' 已成功通過 Sentry 審核並雙軌落庫生效！全體 Agent 下次載入時即可調用此技能！",
        }


def main():
    parser = argparse.ArgumentParser(description="Buzz 技能安全審查與一鍵落庫掛載工具")
    parser.add_argument("--skill", required=True, help="要批准落庫的技能候選名稱 (kebab-case)")
    parser.add_argument("--approver", default="可愛小寶寶 (老闆)", help="批准人稱號")
    parser.add_argument("--workspace", help="專案根目錄")
    parser.add_argument("--user-buzz", help="使用者 .buzz 全域目錄")
    parser.add_argument("--dry-run", action="store_true", help="演練模式 (不進行實體寫入)")

    args = parser.parse_args()

    activator = SkillActivator(
        workspace_root=args.workspace,
        user_buzz_path=args.user_buzz,
    )

    try:
        result = activator.activate(
            skill_name=args.skill,
            approver=args.approver,
            dry_run=args.dry_run,
        )
        print(json.dumps(result, indent=2, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"success": False, "error": str(e)}, indent=2, ensure_ascii=False), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
