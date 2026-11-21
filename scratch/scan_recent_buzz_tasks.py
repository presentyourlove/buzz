import json
import subprocess
import os
import sys

# 獲取 nsec
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

# 1. 取得頻道列表
cmd_channels = [buzz_exe, '--relay', relay, '--private-key', nsec, 'channels', 'list']
res = subprocess.run(cmd_channels, env=env, capture_output=True, text=True, encoding='utf-8', errors='replace')

try:
    c_data = json.loads(res.stdout)
    channels = c_data if isinstance(c_data, list) else c_data.get('channels', [])
    print(f"Total channels: {len(channels)}")
except Exception as e:
    print("Failed to get channels:", e)
    channels = []

# 2. 遍歷每個頻道最近 20 則訊息
recent_threads = []
for c in channels:
    cid = c.get('id') or c.get('channel_id')
    cname = c.get('name') or cid[:8]
    print(f"\nScanning channel: {cname} ({cid})...")
    
    cmd_msgs = [buzz_exe, '--relay', relay, '--private-key', nsec, 'messages', 'get', '--channel', cid, '--limit', '25']
    res_msgs = subprocess.run(cmd_msgs, env=env, capture_output=True, text=True, encoding='utf-8', errors='replace')
    try:
        m_data = json.loads(res_msgs.stdout)
        msgs = m_data if isinstance(m_data, list) else m_data.get('messages', [])
        print(f"  Got {len(msgs)} messages")
        for m in msgs:
            content = m.get('content', '')
            mid = m.get('id', '')
            author = m.get('author', {}).get('name', 'unknown')
            created = m.get('created_at', 0)
            
            # 檢查關鍵詞
            keywords = ['實作', '計畫', 'todo', '未完成', '做到一半', '立項', '斷', '卡住', '審核', 'pending', 'B1', 'B2', 'S3', 'S4', '投資', 'Karpathy', 'Dashboard', 'Worktree', 'ACP']
            if any(k.lower() in content.lower() for k in keywords):
                recent_threads.append({
                    'channel_id': cid,
                    'channel_name': cname,
                    'msg_id': mid,
                    'author': author,
                    'created_at': created,
                    'content': content[:300]
                })
    except Exception as e:
        print(f"  Error reading messages for {cname}: {e}")

print(f"\n=== Found {len(recent_threads)} relevant messages ===")
# 按時間排序
recent_threads.sort(key=lambda x: x['created_at'], reverse=True)
for item in recent_threads[:30]:
    print(f"\n[{item['channel_name']}] {item['author']} (ID: {item['msg_id'][:10]}..., Time: {item['created_at']})")
    print(f"  {item['content']}...")
