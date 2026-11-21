import os
import glob
import re

plans_dir = r"c:\私人\ai agent\buzz\PLANS"

print("=== 1. 盤點 PLANS/ 目錄中所有計畫書現狀 ===")
plans = glob.glob(os.path.join(plans_dir, "*.md"))
plans.sort(key=os.path.getmtime, reverse=True)

for p in plans:
    name = os.path.basename(p)
    mtime = os.path.getmtime(p)
    size = os.path.getsize(p)
    with open(p, "r", encoding="utf-8", errors="ignore") as f:
        content = f.read()
    
    # 找 status, title, author
    status = "未知"
    for line in content.splitlines()[:20]:
        if "status:" in line.lower() or "狀態" in line:
            status = line.strip()
            break
            
    # 檢查是否中斷 (例如是否有尚未完成的標記、句尾中斷)
    last_lines = [l for l in content.splitlines() if l.strip()][-3:]
    is_truncated = False
    if last_lines:
        last_str = " ".join(last_lines)
        if any(last_str.endswith(x) for x in ["、", "，", "：", "->", "|", "```", "（"]):
            is_truncated = True

    print(f"📄 {name}")
    print(f"   狀態: {status} | 大小: {size} bytes | 結尾疑似中斷: {is_truncated}")
    if is_truncated or "pending" in status.lower() or "進行" in status or "草案" in status:
        print(f"   👉 尾部內容摘要: {' / '.join(last_lines)}")
    print()
