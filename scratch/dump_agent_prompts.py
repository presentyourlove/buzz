import json

managed_path = r'C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\managed-agents.json'
with open(managed_path, 'r', encoding='utf-8') as f:
    data = json.load(f)

agents_list = data if isinstance(data, list) else data.get('agents', [])

seen = set()
for ag in agents_list:
    name = ag.get('name')
    if name in seen:
        continue
    seen.add(name)
    prompt = ag.get('system_prompt', '') or ag.get('prompt', '')
    first_lines = "\n".join(prompt.splitlines()[:15])
    print(f"================ Agent: {name} (Model: {ag.get('model')}) ================")
    print(f"Total length: {len(prompt)} chars, {len(prompt.splitlines())} lines")
    print("--- First 15 lines ---")
    print(first_lines)
    print("\n")
