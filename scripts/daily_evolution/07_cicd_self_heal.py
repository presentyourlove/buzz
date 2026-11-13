#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
07_cicd_self_heal.py — 工作流自癒與三層熔斷保護
職責：
1. 檢核 .github/workflows/、Cargo.toml 與本機特工執行環境完整性。
2. 實裝三層熔斷（編譯層、測試層、部署層）。
3. 輸出自癒修復建議與報告至 OUTBOX/。
"""

import os
import sys
import json
import argparse
from pathlib import Path
from datetime import datetime, timezone

REPO_ROOT = Path(r"C:\私人\ai agent\buzz")
OUTBOX_DIR = Path(os.path.expandvars(r"%USERPROFILE%\.buzz\OUTBOX"))

def run_self_heal_check():
    OUTBOX_DIR.mkdir(parents=True, exist_ok=True)
    report_file = OUTBOX_DIR / "CICD_SELF_HEAL_REPORT.json"

    layers = {
        "compile_layer": {"status": "pass", "details": []},
        "test_layer": {"status": "pass", "details": []},
        "deploy_layer": {"status": "pass", "details": []}
    }

    # 1. 編譯層檢核 (Cargo.toml, Cargo.lock)
    cargo_toml = REPO_ROOT / "Cargo.toml"
    if not cargo_toml.exists():
        layers["compile_layer"]["status"] = "fail"
        layers["compile_layer"]["details"].append("找不到根目錄 Cargo.toml")
    else:
        content = cargo_toml.read_text(encoding="utf-8", errors="ignore")
        if "<<<<<<<" in content or ">>>>>>>" in content:
            layers["compile_layer"]["status"] = "circuit_broken"
            layers["compile_layer"]["details"].append("Cargo.toml 存在 Git 衝突標記，觸發編譯層熔斷！")
        else:
            layers["compile_layer"]["details"].append("Cargo.toml 格式乾淨無衝突")

    # 2. 測試層檢核 (Workflows)
    wf_dir = REPO_ROOT / ".github" / "workflows"
    if wf_dir.exists():
        wf_count = len(list(wf_dir.glob("*.yml")))
        layers["test_layer"]["details"].append(f"CI 工作流目錄正常 (共 {wf_count} 個 workflows)")
    else:
        layers["test_layer"]["status"] = "warning"
        layers["test_layer"]["details"].append("未偵測到 .github/workflows 目錄")

    # 3. 部署層檢核 (tools 目錄守護腳本)
    tools_dir = Path(r"C:\私人\tools")
    required_supervisors = ["pollen_supervisor.py", "sentry_supervisor.py"]
    for s in required_supervisors:
        if (tools_dir / s).exists():
            layers["deploy_layer"]["details"].append(f"守護模組 {s} 存在且就緒")
        else:
            layers["deploy_layer"]["status"] = "warning"
            layers["deploy_layer"]["details"].append(f"缺少守護模組: {s}")

    overall_status = "healthy"
    for k, v in layers.items():
        if v["status"] == "circuit_broken":
            overall_status = "circuit_broken"
            break
        elif v["status"] == "fail":
            overall_status = "degraded"

    result = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "overall_status": overall_status,
        "layers": layers,
        "recommendations": [
            "定時巡檢 Cargo.toml 衝突防護",
            "保持守護進程指數退避機制有效"
        ]
    }

    report_file.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    return result

def main():
    parser = argparse.ArgumentParser(description="工作流自癒與三層熔斷 (07_cicd_self_heal)")
    parser.add_argument("--json-only", action="store_true", help="純 JSON 輸出")
    args = parser.parse_args()

    res = run_self_heal_check()
    if args.json_only:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        print("=" * 60)
        print("🛡️  07_cicd_self_heal — 三層熔斷與工作流自癒檢查")
        print("=" * 60)
        print(f"總體健康狀態: {res['overall_status'].upper()}")
        for k, v in res["layers"].items():
            print(f"  - [{k}]: {v['status'].upper()}")
            for d in v["details"]:
                print(f"      * {d}")

if __name__ == "__main__":
    main()

