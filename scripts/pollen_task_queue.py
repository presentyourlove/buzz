#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Pollen 自主任務佇列管理器 (Pollen Task Queue Manager)
依據 POLLEN_MULTI_TASKING_QUEUE_AND_WORKTREE_PLAN.md 實作。
支援主理人一口氣指派多個任務，特工依序自動吞吐連續執行；
並支援方案 2：Git Worktree 沙盒分身動態拉起與自動合流。
"""

import os
import sys
import json
import argparse
from datetime import datetime, timezone
from pathlib import Path

# 引入 worktree 管理器
SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))
import pollen_worktree

QUEUE_FILE = Path(__file__).resolve().parent.parent / ".buzz" / "tasks" / "queue.json"


def now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load_queue() -> dict:
    if not QUEUE_FILE.exists():
        QUEUE_FILE.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "version": "1.0",
            "updated_at": now_iso(),
            "current_task": None,
            "queue": [],
            "completed": []
        }
        save_queue(data)
        return data
    with open(QUEUE_FILE, "r", encoding="utf-8") as f:
        return json.load(f)


def save_queue(data: dict) -> None:
    data["updated_at"] = now_iso()
    QUEUE_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(QUEUE_FILE, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)


def cmd_push(args) -> None:
    data = load_queue()
    task_name = args.name or args.title_pos
    if not task_name:
        print("錯誤：必須提供任務名稱。", file=sys.stderr)
        sys.exit(1)

    task_id = args.id or f"task_{int(datetime.now().timestamp())}"
    task = {
        "id": task_id,
        "name": task_name,
        "description": args.desc or "",
        "priority": args.priority,
        "mode": args.mode,
        "enqueued_at": now_iso(),
        "status": "pending"
    }
    data["queue"].append(task)
    save_queue(data)
    print(f"✅ 任務已加入佇列！ID: {task['id']}, 名稱: {task['name']}, 模式: {task['mode']}, 目前隊列長度: {len(data['queue'])}")


def cmd_list(args) -> None:
    data = load_queue()
    print("=" * 65)
    print(f"📋 Pollen 任務佇列狀態 (最後更新: {data.get('updated_at')})")
    print("=" * 65)
    current = data.get("current_task")
    if current:
        wt_info = f" -> 沙盒目錄: {current.get('worktree_path')}" if current.get('mode') == "worktree" else ""
        print(f"🚀 當前執行中: [{current['id']}] {current['name']} (模式: {current.get('mode', 'queue')}){wt_info}")
    else:
        print("💤 當前執行中: 無 (待命中)")

    queue = data.get("queue", [])
    print(f"\n⏳ 待辦佇列 ({len(queue)} 項):")
    if not queue:
        print("   (隊列為空)")
    for i, t in enumerate(queue, 1):
        print(f"   {i}. [{t['id']}] {t['name']} (優先級: {t.get('priority', 'normal')}, 模式: {t.get('mode', 'queue')})")

    completed = data.get("completed", [])
    print(f"\n✅ 最近完工項目 ({len(completed)} 項):")
    for t in completed[-5:]:
        print(f"   - [{t['id']}] {t['name']} -> {t.get('status')} ({t.get('completed_at', '')})")
    print("=" * 65)


def cmd_next(args) -> None:
    data = load_queue()
    if data.get("current_task"):
        print(f"⚠️ 當前已有任務正在執行中: [{data['current_task']['id']}] {data['current_task']['name']}")
        return

    queue = data.get("queue", [])
    if not queue:
        print("ℹ️ 佇列中沒有待辦任務。")
        return

    next_task = queue.pop(0)
    next_task["started_at"] = now_iso()
    next_task["status"] = "running"

    # 若指定為 Worktree 模式，自動拉起實體沙盒分身
    if next_task.get("mode") == "worktree":
        try:
            wt_res = pollen_worktree.create_worktree(next_task["id"])
            next_task["worktree_path"] = wt_res["path"]
            next_task["worktree_branch"] = wt_res["branch"]
            print(f"🌱 [Worktree 沙盒已啟動] 目錄: {wt_res['path']} (獨立編譯快取已啟用)")
        except Exception as e:
            print(f"⚠️ 建立 Worktree 警告/異常: {e}，回退為常規隊列模式。")
            next_task["mode"] = "queue"

    data["current_task"] = next_task
    save_queue(data)
    print(f"🚀 已啟動下一個任務: [{next_task['id']}] {next_task['name']} (模式: {next_task['mode']})")


def cmd_complete(args) -> None:
    data = load_queue()
    current = data.get("current_task")
    if not current:
        print("⚠️ 目前沒有正在執行的任務。")
        return

    # 若是 Worktree 模式且請求合流
    if current.get("mode") == "worktree" and getattr(args, "merge", True):
        try:
            print(f"🔀 正在將沙盒分支 {current['id']} 自動合流至主分支...")
            pollen_worktree.merge_worktree(current["id"], auto_cleanup=True)
            print("🎉 沙盒分支已成功無損合流並安全清理！")
        except Exception as e:
            print(f"❌ 自動合流失敗: {e}，保留沙盒以供排查。", file=sys.stderr)
            return

    current["status"] = "success"
    current["completed_at"] = now_iso()
    current["summary"] = args.summary or "任務圓滿完成並驗證通過"
    data["completed"].append(current)
    data["current_task"] = None
    save_queue(data)
    print(f"🎉 任務 [{current['id']}] 已標記為完工！")

    # 自動調度下一個任務
    if data.get("queue"):
        cmd_next(args)


def cmd_fail(args) -> None:
    data = load_queue()
    current = data.get("current_task")
    if not current:
        print("⚠️ 目前沒有正在執行的任務。")
        return

    # 若是 Worktree 模式，保留沙盒現場或視 force 標記處理
    if current.get("mode") == "worktree" and getattr(args, "cleanup", False):
        try:
            pollen_worktree.cleanup_worktree(current["id"], force=True)
            print(f"🧹 已清理失敗任務之沙盒現場: {current['id']}")
        except Exception as e:
            print(f"清理沙盒失敗: {e}")

    current["status"] = "failed"
    current["completed_at"] = now_iso()
    current["error"] = args.error or "執行過程中遇到錯誤"
    data["completed"].append(current)
    data["current_task"] = None
    save_queue(data)
    print(f"❌ 任務 [{current['id']}] 已標記為失敗: {current['error']}")


def main():
    parser = argparse.ArgumentParser(description="Pollen 任務佇列與雙軌多工管理器")
    subparsers = parser.add_subparsers(dest="cmd", required=True)

    # push
    p_push = subparsers.add_parser("push", help="將新任務加入佇列")
    p_push.add_argument("title_pos", nargs="?", default=None, help="任務名稱 (位置參數)")
    p_push.add_argument("--name", help="任務名稱 (選項參數)")
    p_push.add_argument("--id", help="指定任務 ID (可選)")
    p_push.add_argument("--desc", help="任務說明")
    p_push.add_argument("--priority", default="normal", choices=["high", "normal", "low"])
    p_push.add_argument("--mode", default="queue", choices=["queue", "worktree"], help="調度模式 (隊列順序執行 或 實體沙盒分身)")

    # list
    subparsers.add_parser("list", help="查看當前佇列狀態")

    # next
    subparsers.add_parser("next", help="取出並開始執行下一個任務")

    # complete
    p_comp = subparsers.add_parser("complete", help="標記當前任務已完成並接續下一個")
    p_comp.add_argument("--summary", help="完工摘要說明")
    p_comp.add_argument("--no-merge", dest="merge", action="store_false", help="針對 worktree 模式略過自動合流")

    # fail
    p_fail = subparsers.add_parser("fail", help="標記當前任務失敗")
    p_fail.add_argument("--error", help="失敗原因說明")
    p_fail.add_argument("--cleanup", action="store_true", help="強制清理對應沙盒")

    args = parser.parse_args()

    if args.cmd == "push":
        cmd_push(args)
    elif args.cmd == "list":
        cmd_list(args)
    elif args.cmd == "next":
        cmd_next(args)
    elif args.cmd == "complete":
        cmd_complete(args)
    elif args.cmd == "fail":
        cmd_fail(args)


if __name__ == "__main__":
    main()
