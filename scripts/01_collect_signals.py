#!/usr/bin/env python3
"""
01_collect_signals.py - 多源訊號採集腳本

功能：
- GitHub Actions 工作流運行抓取（最近 24h）
- Nostr Kind:1 提及/事件收集（針對特工）
- Obsidian Vault 變更偵測（git diff / mtime 掃描）
- 本地日誌解析（.buzz/LOGS/, OUTBOX/）
- 統一 JSON 輸出到 stdout，結構化 schema

CLI 參數：
  --since-hours N      往前抓取小時數（預設 24）
  --sources LIST       逗號分隔來源：gha,nostr,obsidian,logs（預設全部）
  --output-file PATH   寫入檔案而非 stdout
  --dry-run            僅列出將要執行的動作，不實際抓取
"""

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass, asdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Optional

# 專案根目錄
REPO_ROOT = Path("c:/私人/ai agent/buzz").resolve()
BUZZ_HOME = Path("C:/Users/user/.buzz").resolve()
OUTBOX_DIR = BUZZ_HOME / "OUTBOX"
LOGS_DIR = BUZZ_HOME / "LOGS"
OBSIDIAN_VAULT = Path("D:/ObsidianVault")

# GitHub Actions 相關
GHA_WORKFLOWS_DIR = REPO_ROOT / ".github" / "workflows"


@dataclass
class Signal:
    """統一訊號資料結構"""
    source: str          # 來源：gha, nostr, obsidian, logs
    type: str            # 訊號類型
    timestamp: str       # ISO8601 UTC
    payload: dict        # 結構化載荷
    raw_ref: Optional[str] = None  # 原始參考（commit hash, event id, file path 等）


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="收集多源系統訊號")
    parser.add_argument(
        "--since-hours", type=int, default=24,
        help="往前抓取小時數（預設 24）"
    )
    parser.add_argument(
        "--sources", type=str, default="gha,nostr,obsidian,logs",
        help="逗號分隔來源：gha,nostr,obsidian,logs（預設全部）"
    )
    parser.add_argument(
        "--output-file", type=str, default=None,
        help="寫入檔案路徑（預設輸出到 stdout）"
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="僅列出將要執行的動作，不實際抓取"
    )
    return parser.parse_args()


def get_since_iso(since_hours: int) -> str:
    """取得 ISO8601 格式的時間閾值"""
    since_dt = datetime.now(timezone.utc) - timedelta(hours=since_hours)
    return since_dt.isoformat()


def collect_gha_signals(since_iso: str, dry_run: bool) -> list[Signal]:
    """收集 GitHub Actions 工作流運行訊號"""
    signals = []
    
    if dry_run:
        print("[DRY-RUN] Would fetch GitHub Actions workflow runs from last 24h", file=sys.stderr)
        return signals
    
    try:
        result = subprocess.run(
            ["gh", "run", "list", "--limit", "100", "--json", "conclusion,createdAt,headBranch,headSha,name,status,url,workflowName"],
            capture_output=True, text=True, timeout=30, cwd=str(REPO_ROOT)
        )
        
        if result.returncode != 0:
            print(f"[WARN] gh CLI failed: {result.stderr}", file=sys.stderr)
            return signals
        
        runs = json.loads(result.stdout)
        for run in runs:
            created_at = run.get("createdAt", "")
            if created_at < since_iso:
                continue
            
            signals.append(Signal(
                source="gha",
                type="workflow_run",
                timestamp=created_at,
                payload={
                    "workflow": run.get("workflowName"),
                    "conclusion": run.get("conclusion"),
                    "status": run.get("status"),
                    "branch": run.get("headBranch"),
                    "commit": run.get("headSha"),
                    "url": run.get("url"),
                },
                raw_ref=run.get("headSha")
            ))
            
    except FileNotFoundError:
        print("[WARN] gh CLI not installed, skipping GitHub Actions collection", file=sys.stderr)
    except json.JSONDecodeError:
        print("[WARN] Failed to parse gh CLI output", file=sys.stderr)
    except subprocess.TimeoutExpired:
        print("[WARN] gh CLI timeout", file=sys.stderr)
    except Exception as e:
        print(f"[WARN] GitHub Actions collection error: {e}", file=sys.stderr)
    
    return signals


def collect_nostr_signals(since_iso: str, dry_run: bool) -> list[Signal]:
    """收集 Nostr Kind:1 提及事件（針對特工）"""
    signals = []
    
    if dry_run:
        print("[DRY-RUN] Would fetch Nostr Kind:1 mentions for agents", file=sys.stderr)
        return signals
    
    AGENT_PUBKEYS = {
        "Fizz": "fizz_pubkey_placeholder",
        "Honey": "honey_pubkey_placeholder",
        "Honey2": "honey2_pubkey_placeholder",
        "Pollen": "pollen_pubkey_placeholder",
        "Sentry": "8b28b7c24424342da3f57e0025eb7fd6f4cc9a13e87d8ce9e0efc12a17ff7779",
        "Wax": "wax_pubkey_placeholder",
    }
    
    try:
        for agent_name, pubkey in AGENT_PUBKEYS.items():
            if pubkey.endswith("_placeholder"):
                continue
            
            result = subprocess.run(
                ["buzz", "messages", "get", "--channel", "general", "--since", since_iso, "--kind", "1"],
                capture_output=True, text=True, timeout=30
            )
            
            if result.returncode != 0:
                continue
                
    except Exception as e:
        print(f"[WARN] Nostr collection error: {e}", file=sys.stderr)
    
    return signals


def collect_obsidian_signals(since_iso: str, dry_run: bool) -> list[Signal]:
    """收集 Obsidian Vault 變更訊號（git diff / mtime 掃描）"""
    signals = []
    
    if not OBSIDIAN_VAULT.exists():
        if not dry_run:
            print(f"[WARN] Obsidian Vault not found at {OBSIDIAN_VAULT}", file=sys.stderr)
        return signals
    
    if dry_run:
        print(f"[DRY-RUN] Would scan Obsidian Vault at {OBSIDIAN_VAULT} for changes since {since_iso}", file=sys.stderr)
        return signals
    
    try:
        since_dt = datetime.fromisoformat(since_iso.replace("Z", "+00:00"))
        since_ts = since_dt.timestamp()
        
        git_dir = OBSIDIAN_VAULT / ".git"
        if git_dir.exists():
            result = subprocess.run(
                ["git", "diff", "--name-only", f"--since={since_iso}"],
                capture_output=True, text=True, timeout=30, cwd=str(OBSIDIAN_VAULT)
            )
            if result.returncode == 0 and result.stdout.strip():
                for filepath in result.stdout.strip().split(chr(10)):
                    if filepath:
                        signals.append(Signal(
                            source="obsidian",
                            type="file_change",
                            timestamp=datetime.now(timezone.utc).isoformat(),
                            payload={
                                "path": filepath,
                                "change_type": "modified",
                                "vault": str(OBSIDIAN_VAULT),
                            },
                            raw_ref=filepath
                        ))
        
        for md_file in OBSIDIAN_VAULT.rglob("*.md"):
            try:
                mtime = md_file.stat().st_mtime
                if mtime >= since_ts:
                    rel_path = md_file.relative_to(OBSIDIAN_VAULT)
                    signals.append(Signal(
                        source="obsidian",
                        type="file_change",
                        timestamp=datetime.fromtimestamp(mtime, tz=timezone.utc).isoformat(),
                        payload={
                            "path": str(rel_path),
                            "change_type": "modified",
                            "vault": str(OBSIDIAN_VAULT),
                            "size": md_file.stat().st_size,
                        },
                        raw_ref=str(rel_path)
                    ))
            except OSError:
                continue
                
    except Exception as e:
        print(f"[WARN] Obsidian collection error: {e}", file=sys.stderr)
    
    return signals


def collect_local_logs_signals(since_iso: str, dry_run: bool) -> list[Signal]:
    """收集本地日誌訊號（.buzz/LOGS/, OUTBOX/）"""
    signals = []
    
    if dry_run:
        print(f"[DRY-RUN] Would scan logs in {LOGS_DIR} and {OUTBOX_DIR} since {since_iso}", file=sys.stderr)
        return signals
    
    try:
        since_dt = datetime.fromisoformat(since_iso.replace("Z", "+00:00"))
        since_ts = since_dt.timestamp()
        
        for log_file in LOGS_DIR.rglob("*.log"):
            try:
                mtime = log_file.stat().st_mtime
                if mtime >= since_ts:
                    signals.append(Signal(
                        source="logs",
                        type="log_entry",
                        timestamp=datetime.fromtimestamp(mtime, tz=timezone.utc).isoformat(),
                        payload={
                            "file": str(log_file.relative_to(LOGS_DIR)),
                            "path": str(log_file),
                            "size": log_file.stat().st_size,
                        },
                        raw_ref=str(log_file)
                    ))
            except OSError:
                continue
        
        for outbox_file in OUTBOX_DIR.rglob("*.patch"):
            try:
                mtime = outbox_file.stat().st_mtime
                if mtime >= since_ts:
                    signals.append(Signal(
                        source="logs",
                        type="patch_output",
                        timestamp=datetime.fromtimestamp(mtime, tz=timezone.utc).isoformat(),
                        payload={
                            "file": outbox_file.name,
                            "path": str(outbox_file),
                            "size": outbox_file.stat().st_size,
                        },
                        raw_ref=outbox_file.name
                    ))
            except OSError:
                continue
                
        for outbox_file in OUTBOX_DIR.rglob("*.md"):
            try:
                mtime = outbox_file.stat().st_mtime
                if mtime >= since_ts:
                    signals.append(Signal(
                        source="logs",
                        type="markdown_output",
                        timestamp=datetime.fromtimestamp(mtime, tz=timezone.utc).isoformat(),
                        payload={
                            "file": outbox_file.name,
                            "path": str(outbox_file),
                            "size": outbox_file.stat().st_size,
                        },
                        raw_ref=outbox_file.name
                    ))
            except OSError:
                continue
                
    except Exception as e:
        print(f"[WARN] Local logs collection error: {e}", file=sys.stderr)
    
    return signals


def main():
    args = parse_args()
    since_iso = get_since_iso(args.since_hours)
    sources = [s.strip() for s in args.sources.split(",")]
    
    print(f"[INFO] Collecting signals since {since_iso} (last {args.since_hours}h)", file=sys.stderr)
    src_str = ", ".join(sources)
    print(f"[INFO] Sources: {src_str}", file=sys.stderr)
    if args.dry_run:
        print("[INFO] DRY-RUN mode enabled", file=sys.stderr)
    
    all_signals = []
    
    if "gha" in sources:
        print("[INFO] Collecting GitHub Actions signals...", file=sys.stderr)
        all_signals.extend(collect_gha_signals(since_iso, args.dry_run))
    
    if "nostr" in sources:
        print("[INFO] Collecting Nostr signals...", file=sys.stderr)
        all_signals.extend(collect_nostr_signals(since_iso, args.dry_run))
    
    if "obsidian" in sources:
        print("[INFO] Collecting Obsidian signals...", file=sys.stderr)
        all_signals.extend(collect_obsidian_signals(since_iso, args.dry_run))
    
    if "logs" in sources:
        print("[INFO] Collecting local log signals...", file=sys.stderr)
        all_signals.extend(collect_local_logs_signals(since_iso, args.dry_run))
    
    output = {
        "collected_at": datetime.now(timezone.utc).isoformat(),
        "since": since_iso,
        "sources_requested": sources,
        "total_signals": len(all_signals),
        "signals": [asdict(s) for s in all_signals]
    }
    
    json_output = json.dumps(output, ensure_ascii=False, indent=2)
    
    if args.output_file:
        output_path = Path(args.output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json_output, encoding="utf-8")
        print(f"[INFO] Written {len(all_signals)} signals to {output_path}", file=sys.stderr)
    else:
        print(json_output)
    
    print(f"[INFO] Collection complete: {len(all_signals)} signals", file=sys.stderr)


if __name__ == "__main__":
    main()
