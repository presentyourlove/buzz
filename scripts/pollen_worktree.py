#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Pollen 特工 Git Worktree 實體沙盒分身管理工具
用途：提供特工在獨立沙盒工作區進行代碼修改、獨立編譯 (CARGO_TARGET_DIR 解耦) 與安全無損合流。
遵循原則：絕不生成 Git Tag、保障主幹分支安全、Windows Cargo 編譯鎖隔離。
"""

import os
import sys
import re
import json
import shutil
import argparse
import subprocess
from pathlib import Path

# 常數定義
TASK_ID_REGEX = re.compile(r"^[a-zA-Z0-9_\-]+$")
DEFAULT_TARGET_SUBDIR = ".target"


def get_repo_root() -> Path:
    """取得 Git 倉庫根目錄，優先以腳本相對位置定位"""
    script_root = Path(__file__).resolve().parent.parent
    if (script_root / ".git").exists():
        return script_root
    try:
        output = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            text=True,
            encoding="utf-8",
            stderr=subprocess.DEVNULL
        ).strip()
        return Path(output)
    except Exception:
        return script_root


REPO_ROOT = get_repo_root()
WORKTREES_BASE_DIR = REPO_ROOT / ".buzz" / "worktrees"


def validate_task_id(task_id: str) -> None:
    """驗證 task_id 符合安全命名，杜絕 Path Injection"""
    if not TASK_ID_REGEX.match(task_id):
        raise ValueError(f"無效的 task_id '{task_id}'：僅支援英數字、底線與連字號，禁止路徑遍歷。")


def run_git_cmd(args: list[str], cwd: Path = REPO_ROOT) -> subprocess.CompletedProcess:
    """安全執行 Git 命令封裝"""
    return subprocess.run(
        ["git"] + args,
        cwd=cwd,
        text=True,
        encoding="utf-8",
        capture_output=True
    )


def create_worktree(task_id: str, base_branch: str = "HEAD") -> dict:
    """
    建立新的 Worktree 實體沙盒與對應任務分支
    """
    validate_task_id(task_id)
    WORKTREES_BASE_DIR.mkdir(parents=True, exist_ok=True)
    target_path = WORKTREES_BASE_DIR / task_id
    branch_name = f"task/{task_id}"

    if target_path.exists():
        raise RuntimeError(f"沙盒目錄已存在：{target_path}")

    # 檢查分支是否已存在
    check_branch = run_git_cmd(["rev-parse", "--verify", branch_name])
    if check_branch.returncode == 0:
        raise RuntimeError(f"分支 '{branch_name}' 已存在，請指定不同 task_id 或先清理舊分支。")

    # 建立 worktree 及新分支
    res = run_git_cmd(["worktree", "add", "-b", branch_name, str(target_path), base_branch])
    if res.returncode != 0:
        raise RuntimeError(f"建立 Worktree 失敗：{res.stderr.strip()}")

    # 建立專屬 target 目錄，避免與主工作區搶佔 .cargo-lock
    cargo_target = target_path / DEFAULT_TARGET_SUBDIR
    cargo_target.mkdir(parents=True, exist_ok=True)

    return {
        "status": "success",
        "task_id": task_id,
        "branch": branch_name,
        "path": str(target_path),
        "target_dir": str(cargo_target),
        "message": f"成功建立沙盒分支 {branch_name} 於 {target_path}"
    }


def run_in_worktree(task_id: str, cmd_args: list[str]) -> int:
    """
    在指定 Worktree 沙盒內執行命令，並自動注入專屬 CARGO_TARGET_DIR
    """
    validate_task_id(task_id)
    target_path = WORKTREES_BASE_DIR / task_id

    if not target_path.exists():
        raise RuntimeError(f"找不到沙盒目錄：{target_path}")

    # 隔離環境變數設定
    env = os.environ.copy()
    cargo_target = target_path / DEFAULT_TARGET_SUBDIR
    cargo_target.mkdir(parents=True, exist_ok=True)
    env["CARGO_TARGET_DIR"] = str(cargo_target)
    env["AWS_LC_SYS_NO_ASM"] = "1"  # 確保 Windows 下 NASM 解耦相容

    proc = subprocess.run(cmd_args, cwd=target_path, env=env)
    return proc.returncode


def merge_worktree(task_id: str, auto_cleanup: bool = False) -> dict:
    """
    將 Worktree 的任務成果無損合流（Merge）至主分支
    注意：嚴格禁止快轉（--no-ff），保留完整歷史，且絕不觸碰 Git Tag。
    """
    validate_task_id(task_id)
    target_path = WORKTREES_BASE_DIR / task_id
    branch_name = f"task/{task_id}"

    if not target_path.exists():
        raise RuntimeError(f"找不到沙盒目錄：{target_path}")

    # 1. 檢查沙盒內是否有未提交修改
    status_res = run_git_cmd(["status", "--porcelain"], cwd=target_path)
    if status_res.stdout.strip():
        raise RuntimeError(f"沙盒內有未提交的變更，請先在沙盒提交後再執行合流：\n{status_res.stdout}")

    # 2. 在主工作區執行 merge
    commit_msg = f"merge(task): 合流任務 {task_id} 之沙盒成果\n\n- 分支: {branch_name}\n- 沙盒目錄: .buzz/worktrees/{task_id}"
    merge_res = run_git_cmd(["merge", "--no-ff", "-m", commit_msg, branch_name], cwd=REPO_ROOT)

    if merge_res.returncode != 0:
        raise RuntimeError(f"合流失敗（可能存在代碼衝突，請手動檢視）：\n{merge_res.stderr.strip() or merge_res.stdout.strip()}")

    result = {
        "status": "success",
        "task_id": task_id,
        "branch": branch_name,
        "message": f"任務 {task_id} 分支已順利合流至主分支。"
    }

    # 3. 視需要自動清理
    if auto_cleanup:
        clean_res = cleanup_worktree(task_id, force=False)
        result["cleanup"] = clean_res

    return result


def cleanup_worktree(task_id: str, force: bool = False) -> dict:
    """
    清理指定 Worktree 及其對應分支與目錄
    """
    validate_task_id(task_id)
    target_path = WORKTREES_BASE_DIR / task_id
    branch_name = f"task/{task_id}"

    # 1. 移除 worktree
    if target_path.exists():
        remove_args = ["worktree", "remove", str(target_path)]
        if force:
            remove_args.append("--force")
        res = run_git_cmd(remove_args, cwd=REPO_ROOT)
        if res.returncode != 0:
            # 如果 git 移除失敗但 force 為 True，手動砍目錄
            if force:
                shutil.rmtree(target_path, ignore_errors=True)
                run_git_cmd(["worktree", "prune"], cwd=REPO_ROOT)
            else:
                raise RuntimeError(f"移除 Worktree 失敗：{res.stderr.strip()}")

    # 2. 刪除分支
    del_flag = "-D" if force else "-d"
    del_res = run_git_cmd(["branch", del_flag, branch_name], cwd=REPO_ROOT)

    return {
        "status": "success",
        "task_id": task_id,
        "branch": branch_name,
        "message": f"已清理沙盒 {target_path} 及分支 {branch_name}。"
    }


def list_worktrees() -> list[dict]:
    """
    列出所有當前已註冊的 Worktree 狀態
    """
    res = run_git_cmd(["worktree", "list", "--porcelain"], cwd=REPO_ROOT)
    if res.returncode != 0:
        raise RuntimeError(f"查詢 Worktree 清單失敗：{res.stderr.strip()}")

    lines = res.stdout.strip().splitlines()
    worktrees = []
    current_entry = {}

    for line in lines:
        if line.startswith("worktree "):
            if current_entry:
                worktrees.append(current_entry)
            current_entry = {"path": line[len("worktree "):].strip()}
        elif line.startswith("HEAD "):
            current_entry["head"] = line[len("HEAD "):].strip()
        elif line.startswith("branch "):
            current_entry["branch"] = line[len("branch "):].strip()
        elif line == "bare":
            current_entry["bare"] = True
        elif line == "detached":
            current_entry["detached"] = True

    if current_entry:
        worktrees.append(current_entry)

    # 標記是否為 Buzz 專屬沙盒
    buzz_wts = []
    for wt in worktrees:
        p = Path(wt["path"])
        is_buzz = WORKTREES_BASE_DIR.resolve() in p.resolve().parents or p.resolve() == WORKTREES_BASE_DIR.resolve()
        wt["is_buzz_sandbox"] = is_buzz
        if is_buzz:
            wt["task_id"] = p.name
        buzz_wts.append(wt)

    return buzz_wts


def main():
    parser = argparse.ArgumentParser(description="Pollen 特工 Git Worktree 沙盒分身管理工具")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # create
    create_parser = subparsers.add_parser("create", help="建立新沙盒工作區")
    create_parser.add_argument("task_id", help="任務識別碼 (僅英數及連字號)")
    create_parser.add_argument("--base-branch", default="HEAD", help="基線分支或提交點 (預設 HEAD)")

    # run
    run_parser = subparsers.add_parser("run", help="在指定沙盒內執行命令 (自動隔離編譯快取)")
    run_parser.add_argument("task_id", help="任務識別碼")
    run_parser.add_argument("cmd", nargs=argparse.REMAINDER, help="欲執行的命令及其參數")

    # merge
    merge_parser = subparsers.add_parser("merge", help="將沙盒成果合流至主幹分支")
    merge_parser.add_argument("task_id", help="任務識別碼")
    merge_parser.add_argument("--auto-cleanup", action="store_true", help="合流成功後自動清除沙盒與分支")

    # cleanup
    clean_parser = subparsers.add_parser("cleanup", help="清理指定沙盒工作區")
    clean_parser.add_argument("task_id", help="任務識別碼")
    clean_parser.add_argument("--force", action="store_true", help="強制清理未合併之沙盒與分支")

    # list
    subparsers.add_parser("list", help="列出所有當前沙盒工作區")

    args = parser.parse_args()

    try:
        if args.command == "create":
            res = create_worktree(args.task_id, args.base_branch)
            print(json.dumps(res, ensure_ascii=False, indent=2))
        elif args.command == "run":
            if not args.cmd:
                print("請指定欲執行的命令。", file=sys.stderr)
                sys.exit(1)
            code = run_in_worktree(args.task_id, args.cmd)
            sys.exit(code)
        elif args.command == "merge":
            res = merge_worktree(args.task_id, auto_cleanup=args.auto_cleanup)
            print(json.dumps(res, ensure_ascii=False, indent=2))
        elif args.command == "cleanup":
            res = cleanup_worktree(args.task_id, force=args.force)
            print(json.dumps(res, ensure_ascii=False, indent=2))
        elif args.command == "list":
            wts = list_worktrees()
            print(json.dumps(wts, ensure_ascii=False, indent=2))
    except Exception as err:
        print(f"錯誤: {err}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
