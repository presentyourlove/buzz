#!/usr/bin/env python3
"""
04_code_health_check.py - 代碼健康檢查腳本

功能：
- 執行 cargo check、cargo clippy、cargo test
- 捕獲退出碼、stdout、stderr、執行時間
- 輸出結構化 JSON 報告
- 任何檢查失敗時返回非零退出碼（供 CI 整合）
- 支援 --json-only 旗標輸出純淨機器可讀 JSON

CLI 參數：
  --json-only     僅輸出 JSON 到 stdout，抑制 stderr 進度訊息
  --workspace     在 workspace 層級執行（預設開啟）
  --package       指定特定 crate 執行（可重複）
  --output-file   將 JSON 報告寫入檔案而非 stdout
  --timeout       單一命令超時秒數（預設 300 秒）

輸出 JSON 結構：
{
  "timestamp": "2026-11-09T10:30:00Z",
  "checks": [
    {
      "command": "cargo check",
      "success": true,
      "duration_ms": 45231,
      "stdout": "...",
      "stderr": "...",
      "exit_code": 0
    },
    ...
  ],
  "overall_success": true,
  "total_duration_ms": 123456
}
"""

import argparse
import json
import sys
import time
import subprocess
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import List, Optional


@dataclass
class CheckResult:
    """單一檢查命令的執行結果"""
    command: str
    success: bool
    duration_ms: int
    stdout: str
    stderr: str
    exit_code: int


@dataclass
class HealthReport:
    """完整健康檢查報告"""
    timestamp: str
    checks: List[CheckResult]
    overall_success: bool
    total_duration_ms: int


def run_command(
    cmd: List[str],
    cwd: Path,
    timeout: int,
    json_only: bool
) -> CheckResult:
    """執行單一命令並回傳結果"""
    start = time.perf_counter()
    
    if not json_only:
        print(f"[INFO] Running: {' '.join(cmd)}", file=sys.stderr)
    
    try:
        result = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout,
            encoding="utf-8",
            errors="replace"
        )
        duration_ms = int((time.perf_counter() - start) * 1000)
        success = result.returncode == 0
        
        return CheckResult(
            command=" ".join(cmd),
            success=success,
            duration_ms=duration_ms,
            stdout=result.stdout,
            stderr=result.stderr,
            exit_code=result.returncode
        )
    except subprocess.TimeoutExpired:
        duration_ms = int((time.perf_counter() - start) * 1000)
        return CheckResult(
            command=" ".join(cmd),
            success=False,
            duration_ms=duration_ms,
            stdout="",
            stderr=f"Command timed out after {timeout} seconds",
            exit_code=-1
        )
    except Exception as e:
        duration_ms = int((time.perf_counter() - start) * 1000)
        return CheckResult(
            command=" ".join(cmd),
            success=False,
            duration_ms=duration_ms,
            stdout="",
            stderr=str(e),
            exit_code=-1
        )


def build_cargo_cmd(
    base_cmd: str,
    workspace: bool,
    packages: Optional[List[str]]
) -> List[str]:
    """構建 cargo 命令列表"""
    cmd = ["cargo", base_cmd]
    if workspace:
        cmd.append("--workspace")
    if packages:
        for pkg in packages:
            cmd.extend(["--package", pkg])
    return cmd


def main():
    parser = argparse.ArgumentParser(
        description="代碼健康檢查：cargo check / clippy / test",
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--json-only",
        action="store_true",
        help="僅輸出 JSON 到 stdout，抑制進度訊息"
    )
    parser.add_argument(
        "--workspace",
        action="store_true",
        default=True,
        help="在 workspace 層級執行（預設開啟）"
    )
    parser.add_argument(
        "--no-workspace",
        action="store_false",
        dest="workspace",
        help="不使用 --workspace 旗標"
    )
    parser.add_argument(
        "--package",
        "-p",
        action="append",
        help="指定特定 crate 執行（可重複）"
    )
    parser.add_argument(
        "--output-file",
        "-o",
        type=Path,
        help="將 JSON 報告寫入檔案而非 stdout"
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=300,
        help="單一命令超時秒數（預設 300）"
    )
    parser.add_argument(
        "--skip",
        action="append",
        choices=["check", "clippy", "test"],
        help="跳過特定檢查（可重複）"
    )
    
    args = parser.parse_args()
    
    # 專案根目錄
    project_root = Path(__file__).resolve().parent.parent.parent
    
    # 確認 Cargo.toml 存在
    if not (project_root / "Cargo.toml").exists():
        error_msg = f"Cargo.toml not found at {project_root}"
        if args.json_only:
            report = HealthReport(
                timestamp=datetime.now(timezone.utc).isoformat(),
                checks=[],
                overall_success=False,
                total_duration_ms=0
            )
            print(json.dumps(asdict(report), ensure_ascii=False, indent=2))
        else:
            print(f"[ERROR] {error_msg}", file=sys.stderr)
        sys.exit(1)
    
    # 要執行的檢查項目
    checks_to_run = []
    if "check" not in (args.skip or []):
        checks_to_run.append(("cargo check", build_cargo_cmd("check", args.workspace, args.package)))
    if "clippy" not in (args.skip or []):
        checks_to_run.append(("cargo clippy", build_cargo_cmd("clippy", args.workspace, args.package) + ["--", "-D", "warnings"]))
    if "test" not in (args.skip or []):
        checks_to_run.append(("cargo test", build_cargo_cmd("test", args.workspace, args.package)))
    
    if not checks_to_run:
        if not args.json_only:
            print("[WARN] All checks skipped", file=sys.stderr)
        report = HealthReport(
            timestamp=datetime.now(timezone.utc).isoformat(),
            checks=[],
            overall_success=True,
            total_duration_ms=0
        )
        output_json = json.dumps(asdict(report), ensure_ascii=False, indent=2)
        if args.output_file:
            args.output_file.write_text(output_json, encoding="utf-8")
        else:
            print(output_json)
        return
    
    # 執行所有檢查
    total_start = time.perf_counter()
    results: List[CheckResult] = []
    
    for name, cmd in checks_to_run:
        result = run_command(cmd, project_root, args.timeout, args.json_only)
        results.append(result)
        
        if not args.json_only:
            status = "PASS" if result.success else "FAIL"
            print(f"[INFO] {name}: {status} ({result.duration_ms}ms)", file=sys.stderr)
            if not result.success and result.stderr:
                # 只顯示前幾行錯誤
                err_lines = result.stderr.strip().split("\n")
                for line in err_lines[:10]:
                    print(f"       {line}", file=sys.stderr)
                if len(err_lines) > 10:
                    print(f"       ... ({len(err_lines) - 10} more lines)", file=sys.stderr)
    
    total_duration_ms = int((time.perf_counter() - total_start) * 1000)
    overall_success = all(r.success for r in results)
    
    # 建立報告
    report = HealthReport(
        timestamp=datetime.now(timezone.utc).isoformat(),
        checks=results,
        overall_success=overall_success,
        total_duration_ms=total_duration_ms
    )
    
    # 輸出 JSON
    output_json = json.dumps(asdict(report), ensure_ascii=False, indent=2)
    
    if args.output_file:
        args.output_file.parent.mkdir(parents=True, exist_ok=True)
        args.output_file.write_text(output_json, encoding="utf-8")
        if not args.json_only:
            print(f"[INFO] Report written to {args.output_file}", file=sys.stderr)
    else:
        print(output_json)
    
    # 非 JSON 模式下的摘要
    if not args.json_only:
        print(f"\n[INFO] Health check complete: {'ALL PASS' if overall_success else 'SOME FAILED'}", file=sys.stderr)
        print(f"[INFO] Total duration: {total_duration_ms}ms", file=sys.stderr)
        for r in results:
            status = "✓" if r.success else "✗"
            print(f"  {status} {r.command}: {r.duration_ms}ms", file=sys.stderr)
    
    # 返回適當的退出碼
    sys.exit(0 if overall_success else 1)


if __name__ == "__main__":
    main()
