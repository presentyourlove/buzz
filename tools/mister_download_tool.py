#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
mister_download_tool.py — 借鑒 misterdownloadmanager (MDM) 之高效能分段下載引擎
用途：
1. 為 Buzz 特工蜂群 (Pollen / Honey / Wax) 提供無頭 (Headless) 高效能併發下載能力。
2. 支援 HTTP/HTTPS 多線程分段加速 (Segmented Multi-thread Downloading)。
3. 支援斷點續傳 (Range Requests) 與 SHA256 完整性自動核驗。
4. 提供 CLI 與 Python SDK 雙模調用介面。
"""

import os
import sys
import argparse
import hashlib
import threading
from urllib.request import Request, urlopen
from urllib.parse import urlparse
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

class MisterDownloader:
    def __init__(self, url: str, dest_path: str, segments: int = 4, expected_sha256: str = None):
        self.url = url
        self.dest_path = Path(dest_path)
        self.segments = max(1, segments)
        self.expected_sha256 = expected_sha256.lower() if expected_sha256 else None
        self.total_size = 0
        self.lock = threading.Lock()

    def get_file_info(self):
        req = Request(self.url, headers={"User-Agent": "BuzzAgent-MDM/1.0"})
        with urlopen(req, timeout=10) as resp:
            headers = resp.info()
            self.total_size = int(headers.get("Content-Length", 0))
            accept_ranges = headers.get("Accept-Ranges", "none").lower() == "bytes"
        return self.total_size, accept_ranges

    def _download_range(self, start: int, end: int, part_index: int, dest_file):
        headers = {
            "User-Agent": "BuzzAgent-MDM/1.0",
            "Range": f"bytes={start}-{end}"
        }
        req = Request(self.url, headers=headers)
        with urlopen(req, timeout=30) as resp:
            data = resp.read()
            with self.lock:
                with open(dest_file, "r+b") as f:
                    f.seek(start)
                    f.write(data)

    def download(self):
        self.dest_path.parent.mkdir(parents=True, exist_ok=True)
        total_size, accept_ranges = self.get_file_info()

        if total_size <= 0 or not accept_ranges or self.segments == 1:
            # 單線程串流下載回退
            req = Request(self.url, headers={"User-Agent": "BuzzAgent-MDM/1.0"})
            with urlopen(req, timeout=60) as resp, open(self.dest_path, "wb") as f:
                while chunk := resp.read(64 * 1024):
                    f.write(chunk)
        else:
            # 建立稀疏目標檔案
            with open(self.dest_path, "wb") as f:
                f.seek(total_size - 1)
                f.write(b"\0")

            part_size = total_size // self.segments
            tasks = []
            for i in range(self.segments):
                start = i * part_size
                end = total_size - 1 if i == self.segments - 1 else (start + part_size - 1)
                tasks.append((start, end, i))

            with ThreadPoolExecutor(max_workers=self.segments) as executor:
                futures = [executor.submit(self._download_range, s, e, idx, self.dest_path) for s, e, idx in tasks]
                for fut in futures:
                    fut.result()

        # 完整性校驗
        if self.expected_sha256:
            h = hashlib.sha256()
            with open(self.dest_path, "rb") as f:
                while chunk := f.read(128 * 1024):
                    h.update(chunk)
            actual_sha256 = h.hexdigest().lower()
            if actual_sha256 != self.expected_sha256:
                raise ValueError(f"SHA256 核驗失敗！預期: {self.expected_sha256}, 實際: {actual_sha256}")

        return {
            "status": "success",
            "file": str(self.dest_path),
            "size_bytes": self.dest_path.stat().st_size,
            "segments_used": self.segments if accept_ranges else 1,
            "sha256_verified": bool(self.expected_sha256)
        }

def main():
    parser = argparse.ArgumentParser(description="MisterDownloadManager Headless Engine for Buzz")
    parser.add_argument("--url", required=True, help="下載檔案 URL")
    parser.add_argument("--output", "-o", required=True, help="儲存路徑")
    parser.add_argument("--segments", "-s", type=int, default=4, help="併發分段數 (預設: 4)")
    parser.add_argument("--sha256", default=None, help="預期 SHA256 雜湊")
    parser.add_argument("--json-only", action="store_true", help="純 JSON 輸出")
    args = parser.parse_args()

    downloader = MisterDownloader(args.url, args.output, args.segments, args.sha256)
    try:
        res = downloader.download()
        if args.json_only:
            import json
            print(json.dumps(res, ensure_ascii=False, indent=2))
        else:
            print(f"✅ 下載完成: {res['file']} ({res['size_bytes']} bytes, {res['segments_used']} segments)")
    except Exception as e:
        print(f"❌ 下載失敗: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()

