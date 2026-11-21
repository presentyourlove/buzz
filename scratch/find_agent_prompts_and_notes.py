import os
import re

buzz_dir = r"c:\私人\ai agent\buzz"

print("=== 1. 搜尋檔名包含 prompt, persona, agent, note, guide, format 的檔案 ===")
matched_files = []
for root, dirs, files in os.walk(buzz_dir):
    # 跳過 node_modules, target, .git, buzz-target
    dirs[:] = [d for d in dirs if d not in ('node_modules', 'target', '.git', 'buzz-target', '.cargo-targets')]
    for f in files:
        f_lower = f.lower()
        if any(k in f_lower for k in ['prompt', 'persona', 'agent', 'note', 'guide', 'format', 'manifest', 'rule']):
            full = os.path.join(root, f)
            matched_files.append(full)

print(f"找到 {len(matched_files)} 個檔名相符的檔案，列出前 40 個：")
for p in matched_files[:40]:
    print(" ", os.path.relpath(p, buzz_dir))

print("\n=== 2. 全文搜尋筆記中關於格式、全域 prompt、專案結構、skill 目錄的關鍵字 ===")
keywords = ["專案結構", "全域", "global_prompt", "system_prompt", "skill的目錄", "skills目錄", "每次工作前", "prompt", "persona"]

note_matches = []
search_exts = {'.md', '.txt', '.rs', '.json', '.toml', '.py'}
for root, dirs, files in os.walk(buzz_dir):
    dirs[:] = [d for d in dirs if d not in ('node_modules', 'target', '.git', 'buzz-target', '.cargo-targets')]
    for f in files:
        ext = os.path.splitext(f)[1].lower()
        if ext in search_exts:
            full = os.path.join(root, f)
            try:
                with open(full, 'r', encoding='utf-8', errors='ignore') as fp:
                    content = fp.read()
                    # 搜尋特定組合
                    if ("專案結構" in content or "skill" in content) and ("prompt" in content or "全域" in content or "persona" in content):
                        # 計算符合度
                        score = sum(1 for kw in keywords if kw in content)
                        if score >= 3:
                            rel = os.path.relpath(full, buzz_dir)
                            note_matches.append((score, rel, full))
            except Exception as e:
                pass

note_matches.sort(key=lambda x: x[0], reverse=True)
print(f"找到符合關鍵字組合的檔案 {len(note_matches)} 個，評分最高前 25 個：")
for score, rel, full in note_matches[:25]:
    print(f" [Score: {score}] {rel}")
