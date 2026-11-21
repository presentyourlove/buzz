import os
import sys

buzz_dir = r"c:\私人\ai agent\buzz"
ignore_dirs = {
    'node_modules', 'target', '.git', 'buzz-target', '.cargo-targets',
    '%SystemDrive%', 'dist', 'build', '.vscode', '.idea', 'bin', 'buzz-target',
    'benchmarks', 'perf'
}

print("=== Scanning directories in buzz ===", flush=True)

search_exts = {'.md', '.txt', '.rs', '.json', '.toml', '.py'}
keywords = ["專案結構", "全域", "global_prompt", "system_prompt", "skill的目錄", "skills目錄", "每次工作前", "prompt", "persona"]

note_matches = []
matched_files = []

for root, dirs, files in os.walk(buzz_dir):
    dirs[:] = [d for d in dirs if d not in ignore_dirs and not d.startswith('.git')]
    
    for f in files:
        f_lower = f.lower()
        full = os.path.join(root, f)
        
        # 檔名相符
        if any(k in f_lower for k in ['prompt', 'persona', 'agent', 'note', 'guide', 'rule']):
            rel = os.path.relpath(full, buzz_dir)
            matched_files.append(rel)
            
        ext = os.path.splitext(f)[1].lower()
        if ext in search_exts:
            try:
                # 限制讀取大小避免卡大檔案
                size = os.path.getsize(full)
                if size > 2 * 1024 * 1024:
                    continue
                with open(full, 'r', encoding='utf-8', errors='ignore') as fp:
                    content = fp.read()
                    score = 0
                    for kw in keywords:
                        if kw in content:
                            score += 1
                    if score >= 3:
                        rel = os.path.relpath(full, buzz_dir)
                        note_matches.append((score, rel))
            except Exception:
                pass

print(f"Matched filenames: {len(matched_files)}", flush=True)
for p in matched_files[:30]:
    print("  FILE:", p, flush=True)

print(f"\nMatched content notes: {len(note_matches)}", flush=True)
note_matches.sort(key=lambda x: x[0], reverse=True)
for score, rel in note_matches[:30]:
    print(f"  [Score: {score}] {rel}", flush=True)
