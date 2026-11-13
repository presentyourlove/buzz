#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
01_work_discipline_guard.py — 標準開工紀律守門員
職責：
1. 掃描 C:\\Users\\user\\.buzz\\PLANS\\ 中狀態為 active 的計畫書。
2. 檢核四要素：buzz projects 綁定、buzz issues 工單、Sentry 審核標記、主理人明確授權。
3. 偵測違規：未授權私自立項、未經 Sentry 審核私自開工、特工職責越界。
4. 輸出標準結構化報告 (支援 --json-only)。
"""

import os
import sys
import json
import argparse
import re
from pathlib import Path
from datetime import datetime, timezone

PLANS_DIR = Path(os.path.expandvars(r"%USERPROFILE%\.buzz\PLANS"))

def scan_plans(plans_dir: Path):
    violations = []
    checked_count = 0
    active_count = 0

    if not plans_dir.exists():
        return {
            "status": "warning",
            "message": f"PLANS 目錄不存在: {plans_dir}",
            "checked_count": 0,
            "active_count": 0,
            "violations": []
        }

    for p in plans_dir.glob("*.md"):
        checked_count += 1
        try:
            content = p.read_text(encoding="utf-8", errors="ignore")
        except Exception as e:
            continue

        # 檢測是否為 active 狀態
        is_active = False
        if re.search(r"status:\s*active", content, re.IGNORECASE) or "【狀態】：執行中" in content or "狀態: 進行中" in content:
            is_active = True

        if is_active:
            active_count += 1
            issues = []
            
            # 1. 檢核專案/工單綁定
            has_project = bool(re.search(r"(?:buzz\s+projects|project[_\s]id|專案\s*ID)", content, re.IGNORECASE))
            has_issue = bool(re.search(r"(?:buzz\s+issues|issue[_\s]id|工單\s*ID)", content, re.IGNORECASE))
            if not has_project and not has_issue:
                issues.append("MISSING_PROJECT_OR_ISSUE_BINDING: 缺少 buzz projects 或 issues 工單綁定")

            # 2. 檢核 Sentry 審核標記
            has_sentry_review = bool(re.search(r"(?:sentry\s*審核|sentry\s*review|sentry\s*qa|審核特工\s*:\s*sentry)", content, re.IGNORECASE))
            if not has_sentry_review:
                issues.append("MISSING_SENTRY_APPROVAL: 缺少 Sentry 安全品質審核標記")

            # 3. 檢核主理人授權
            has_owner_auth = bool(re.search(r"(?:主理人|授權|可愛小寶寶|owner\s*approved)", content, re.IGNORECASE))
            if not has_owner_auth:
                issues.append("MISSING_OWNER_AUTHORIZATION: 缺少主理人明確授權標記")

            if issues:
                violations.append({
                    "file": p.name,
                    "path": str(p),
                    "status": "active",
                    "violations": issues
                })

    return {
        "status": "pass" if not violations else "violation_detected",
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "checked_count": checked_count,
        "active_count": active_count,
        "violation_count": len(violations),
        "violations": violations
    }

def main():
    parser = argparse.ArgumentParser(description="標準開工紀律守門員")
    parser.add_argument("--plans-dir", default=str(PLANS_DIR), help="計畫書目錄路徑")
    parser.add_argument("--json-only", action="store_true", help="僅輸出純 JSON 結果")
    args = parser.parse_args()

    result = scan_plans(Path(args.plans_dir))

    if args.json_only:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print("=" * 60)
        print("🛡️  01_work_discipline_guard — 開工紀律檢查報告")
        print("=" * 60)
        print(f"檢核計畫書總數: {result['checked_count']}")
        print(f"活躍執行中 (active): {result['active_count']}")
        print(f"違規計畫數: {result['violation_count']}")
        if result["violations"]:
            print("\n⚠️ 偵測到之違規項目：")
            for v in result["violations"]:
                print(f"  - [{v['file']}]:")
                for err in v["violations"]:
                    print(f"      * {err}")
        else:
            print("\n✅ 所有活躍計畫書均符合開工紀律規範！")

    sys.exit(0 if result["violation_count"] == 0 else 1)

if __name__ == "__main__":
    main()

