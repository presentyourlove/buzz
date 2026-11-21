import os

buzz_dir = r"c:\私人\ai agent\buzz"
search_dirs = [
    os.path.join(buzz_dir, "docs"),
    os.path.join(buzz_dir, "PLANS"),
    os.path.join(buzz_dir, "GUIDES"),
    os.path.join(buzz_dir, "RESEARCH"),
    buzz_dir
]

keywords = ["專案結構", "全域", "global", "prompt", "每次工作前", "目錄", "格式"]

matches = []

for s_dir in search_dirs:
    if not os.path.exists(s_dir):
        continue
    for root, dirs, files in os.walk(s_dir):
        dirs[:] = [d for d in dirs if d not in ('node_modules', 'target', '.git', 'buzz-target')]
        for f in files:
            if f.endswith('.md') or f.endswith('.txt'):
                p = os.path.join(root, f)
                try:
                    with open(p, 'r', encoding='utf-8', errors='ignore') as fp:
                        lines = fp.readlines()
                        for i, line in enumerate(lines):
                            if "專案結構" in line or ("全域" in line and "prompt" in line.lower()) or "每次工作前" in line:
                                matches.append((p, i + 1, line.strip()))
                except Exception:
                    pass

print(f"Total matching lines: {len(matches)}")
for path, line_no, text in matches[:40]:
    rel = os.path.relpath(path, buzz_dir)
    print(f"{rel}:{line_no} -> {text}")
