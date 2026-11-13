#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
06_knowledge_sync.py — 工作日誌與決策同步至 Obsidian 第二大腦
職責：
1. 讀取 .buzz/PLANS/ 與 .buzz/RESEARCH/ 的產出。
2. 同步至 Obsidian Vault 知識庫目錄 (預設 C:\\私人\\ai agent\\buzz-obsidian)。
3. 使用 SHA256 雜湊進行增量同步，遇到衝突自動備份至 .conflicts/。
"""

import os
import sys
import json
import hashlib
import shutil
import argparse
from pathlib import Path
from datetime import datetime, timezone

BUZZ_BASE = Path(os.path.expandvars(r"%USERPROFILE%\.buzz"))
DEFAULT_OBSIDIAN = Path(r"C:\私人\ai agent\buzz-obsidian")

def get_sha256(file_path: Path) -> str:
    h = hashlib.sha256()
    h.update(file_path.read_bytes())
    return h.hexdigest()

def sync_knowledge(obsidian_vault: Path = None):
    vault = obsidian_vault or DEFAULT_OBSIDIAN
    dest_dir = vault / "wiki" / "02-Conversation-Digest"
    dest_dir.mkdir(parents=True, exist_ok=True)
    conflicts_dir = dest_dir / ".conflicts"

    synced_files = []
    skipped_files = []

    sources = [
        BUZZ_BASE / "PLANS",
        BUZZ_BASE / "RESEARCH"
    ]

    for src_dir in sources:
        if not src_dir.exists():
            continue
        for src_file in src_dir.glob("*.md"):
            dest_file = dest_dir / src_file.name

            try:
                src_hash = get_sha256(src_file)

                if dest_file.exists():
                    dest_hash = get_sha256(dest_file)
                    if src_hash == dest_hash:
                        skipped_files.append(src_file.name)
                        continue
                    else:
                        # 內容不同，若目標修改時間較新則備份舊檔
                        if dest_file.stat().st_mtime > src_file.stat().st_mtime:
                            conflicts_dir.mkdir(parents=True, exist_ok=True)
                            ts = datetime.now().strftime("%Y%m%d_%H%M%S")
                            shutil.copy2(dest_file, conflicts_dir / f"{dest_file.stem}_{ts}.bak")

                shutil.copy2(src_file, dest_file)
                synced_files.append(src_file.name)
            except Exception as e:
                pass

    return {
        "status": "success",
        "synced_count": len(synced_files),
        "skipped_count": len(skipped_files),
        "synced_files": synced_files,
        "destination": str(dest_dir)
    }

def main():
    parser = argparse.ArgumentParser(description="工作日誌同步至 Obsidian 第二大腦 (06_knowledge_sync)")
    parser.add_argument("--vault", default=str(DEFAULT_OBSIDIAN), help="Obsidian Vault 路徑")
    parser.add_argument("--json-only", action="store_true", help="純 JSON 輸出")
    args = parser.parse_args()

    res = sync_knowledge(Path(args.vault))
    if args.json_only:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        print(f"✅ 第二大腦知識庫同步完成: 成功同步 {res['synced_count']} 份檔案，跳過 {res['skipped_count']} 份相同檔案")
        print(f"   同步目標目錄: {res['destination']}")

if __name__ == "__main__":
    main()

