import os
import json

buzz_dir = r"c:\私人\ai agent\buzz"
ignore_dirs = {'node_modules', 'target', '.git', 'buzz-target', '.cargo-targets', 'dist', 'build'}

print("=== 1. 搜尋所有 .persona.md 檔案 ===")
personas = []
for root, dirs, files in os.walk(buzz_dir):
    dirs[:] = [d for d in dirs if d not in ignore_dirs]
    for f in files:
        if f.endswith('.persona.md') or f.endswith('.agent.json'):
            personas.append(os.path.join(root, f))

for p in personas:
    print("  FOUND:", os.path.relpath(p, buzz_dir))

print("\n=== 2. 搜尋 managed-agents.json 中的所有 agent ===")
managed_path = r'C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\managed-agents.json'
if os.path.exists(managed_path):
    with open(managed_path, 'r', encoding='utf-8') as f:
        data = json.load(f)
    agents_list = data if isinstance(data, list) else data.get('agents', [])
    print(f"Total entries in managed-agents.json: {len(agents_list)}")
    unique_names = {}
    for idx, ag in enumerate(agents_list):
        name = ag.get('name') or ag.get('id')
        p_len = len(ag.get('system_prompt', '') or ag.get('prompt', ''))
        unique_names[name] = unique_names.get(name, 0) + 1
        print(f"  [{idx}] Name: {name}, Model: {ag.get('model')}, Prompt Length: {p_len} chars")
    print("Unique agent names:", unique_names)
else:
    print("managed-agents.json NOT FOUND!")
