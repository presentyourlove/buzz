import json
import subprocess
import os

with open(r'C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\managed-agents.json', 'r', encoding='utf-8') as f:
    agents = json.load(f)

agent = next(a for a in agents if a.get('private_key_nsec'))
nsec = agent['private_key_nsec']
raw_auth_tag = agent.get('auth_tag', '')

buzz_exe = r'C:\Users\user\AppData\Local\Buzz\buzz.exe'
relay = 'https://presentyourlove.communities.buzz.xyz'

env = os.environ.copy()
if raw_auth_tag:
    env['BUZZ_AUTH_TAG'] = raw_auth_tag

# 讀取 DM 頻道
dm_channel = '312a75a2-6ef6-4844-8c4c-26b28e810b74'
cmd = [buzz_exe, '--relay', relay, '--private-key', nsec, 'messages', 'get', '--channel', dm_channel, '--limit', '10']
res = subprocess.run(cmd, env=env, capture_output=True, text=True, encoding='utf-8', errors='replace')
print("=== DM Messages ===")
print(res.stdout)
