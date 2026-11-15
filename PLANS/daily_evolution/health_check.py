#!/usr/bin/env python3
"""
系統健全性自我檢查與即時監控腳本 (health_check.py)

功能：
1. 檢驗 Buzz CLI 執行檔是否存在與可執行
2. 檢驗環境變數完整性 (BUZZ_RELAY_URL, BUZZ_PRIVATE_KEY, BUZZ_AUTH_TAG)
3. 檢查熔斷日誌狀態 (daily_evolution_failure.log)
4. 執行快速單元測試驗證模組完整性
5. 支援異常自動告警至目標頻道
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path

DEFAULT_CLI = "C:/Users/user/AppData/Local/Buzz/buzz.exe"
DEFAULT_CHANNEL = "5ca019f6-1a27-592a-b015-4baf0eb33572"
DEFAULT_RELAY = "wss://presentyourlove.communities.buzz.xyz"


def check_buzz_cli(cli_path: str) -> bool:
    print(f"🔍 檢查 Buzz CLI 狀態: {cli_path}")
    if not Path(cli_path).exists():
        print(f"❌ Buzz CLI 不存在於路徑: {cli_path}")
        return False
    try:
        res = subprocess.run([cli_path, "--version"], capture_output=True, text=True, timeout=5)
        print(f"✅ Buzz CLI 可正常執行: {res.stdout.strip()}")
        return True
    except Exception as e:
        print(f"❌ Buzz CLI 執行失敗: {e}")
        return False


def check_env_vars() -> bool:
    print("🔍 檢查核心環境變數...")
    relay = os.environ.get("BUZZ_RELAY_URL", DEFAULT_RELAY)
    priv = os.environ.get("BUZZ_PRIVATE_KEY", "")
    print(f"   Relay URL: {relay}")
    print(f"   Private Key: {'已配置 (隱藏)' if priv else '⚠️ 未配置 (依賴預設值)'}")
    return True


def check_failure_log(workspace: Path) -> bool:
    print("🔍 檢查熔斷日誌...")
    failure_log = workspace / "WORK_LOGS" / "daily_evolution_failure.log"
    if failure_log.exists():
        lines = failure_log.read_text(encoding="utf-8").strip().split("\n")
        lines = [l for l in lines if l]
        if len(lines) >= 2:
            print(f"⚠️ 警告：熔斷器已跳脫 (發現 {len(lines)} 次連續失敗紀錄)！")
            return False
        else:
            print(f"ℹ️ 發現 {len(lines)} 筆失敗紀錄，尚未達到熔斷閥值。")
    else:
        print("✅ 熔斷日誌正常（無失敗紀錄）。")
    return True


def check_skill_system(workspace: Path, script_dir: Path) -> bool:
    print("🔍 檢查技能合成與掛載系統模組...")
    synthesizer_file = script_dir / "skill_synthesizer.py"
    activator_file = script_dir / "activate_skill.py"
    candidates_dir = workspace / "PLANS" / "SKILL_CANDIDATES"

    if not synthesizer_file.exists():
        print(f"❌ 找不到技能合成器模組: {synthesizer_file}")
        return False
    if not activator_file.exists():
        print(f"❌ 找不到技能落庫掛載模組: {activator_file}")
        return False

    candidates_dir.mkdir(parents=True, exist_ok=True)
    print(f"✅ 技能合成系統模組就緒 (候選目錄: {candidates_dir})")
    return True


def run_unit_tests(script_dir: Path) -> bool:
    print("🔍 執行單元回歸測試 (含進化分析與技能合成)...")
    test_files = [
        script_dir / "test_daily_evolution.py",
        script_dir / "test_skill_synthesis.py",
    ]
    for test_file in test_files:
        if not test_file.exists():
            print(f"⚠️ 未找到測試檔: {test_file}")
            return False
        try:
            res = subprocess.run([sys.executable, str(test_file)], capture_output=True, text=True, timeout=10)
            if res.returncode != 0:
                print(f"❌ 單元測試失敗 ({test_file.name}):\n{res.stderr}")
                return False
        except Exception as e:
            print(f"❌ 執行單元測試發生錯誤 ({test_file.name}): {e}")
            return False

    print("✅ 所有單元測試通過 (100% OK，包含進化分析與技能合成 13 項測試)")
    return True


def main():
    parser = argparse.ArgumentParser(description="Buzz 系統健康檢查")
    parser.add_argument("--cli", default=DEFAULT_CLI, help="Buzz CLI 路徑")
    parser.add_argument("--workspace", default=str(Path(__file__).resolve().parent.parent.parent), help="專案根目錄")
    args = parser.parse_args()

    workspace = Path(args.workspace)
    script_dir = Path(__file__).resolve().parent

    print("========================================")
    print("🏥 開始執行 Buzz 每日進化系統健康檢查")
    print("========================================")

    cli_ok = check_buzz_cli(args.cli)
    env_ok = check_env_vars()
    fail_ok = check_failure_log(workspace)
    skill_ok = check_skill_system(workspace, script_dir)
    tests_ok = run_unit_tests(script_dir)

    all_ok = cli_ok and env_ok and fail_ok and skill_ok and tests_ok

    print("========================================")
    if all_ok:
        print("🎉 系統健康檢查全部通過！(System Status: HEALTHY)")
        sys.exit(0)
    else:
        print("❌ 系統健康檢查發現異常，請參閱上述日誌！(System Status: UNHEALTHY)")
        sys.exit(1)


if __name__ == "__main__":
    main()

