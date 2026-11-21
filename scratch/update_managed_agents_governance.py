import json
import time
import shutil
import os

path = r'C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\managed-agents.json'
ts = int(time.time())
bak_path = f'{path}.bak_phase58_{ts}'

# 1. 備份
shutil.copyfile(path, bak_path)
print(f"Backed up to: {bak_path}")

# 2. 讀取
with open(path, 'r', encoding='utf-8') as f:
    data = json.load(f)

# 核心注入文本
GOVERNANCE_AND_CAPABILITY_PROMPT = """

---
### 【特工治理三原則與系統真實底層清冊 (Phase 58 核心鐵律)】
1. **底層真實資產掌握 (System Reality Manifest)**：
   - 【已就緒】`Proactive Trigger Orchestrator`：運行於 Port 8080（`crates/buzz-agent/src/triggers/`），專職 Cron 與事件排程。嚴禁重新設計排程底層，一律調用既有 8080 服務。
   - 【已就緒】`buzz pack triple-gate`：包含 Syntax、Semantic、Fingerprint Circuit Breaker 60 秒三閘防線。嚴禁重複設計 Gate 1/2/3。
   - 【已就緒】`buzz-acp` RBAC 安全沙箱：角色權限安全控管。
   - 【已就緒】`Vite 4173 Dashboard`：即時 Telemetry 遙測與 6 特工活力圖。
   - 【待實作】`B1: comm-quality-guard`：代碼庫為 0 行，需以 MVP 規格移交後台。
   - 【鐵律】：嚴禁重複規劃已存在之設施；嚴禁宣稱未實作之技能已上線！

2. **MVP 漸進交付核心鐵律 (Anti-Proliferation & No Bureaucracy)**：
   - 嚴禁跨回合無限審核套娃與空想巨幅藍圖。
   - 任何新提議必須在單次回合內產出 30 行以內的「最小可行原型 (MVP)」規格（僅含：目標痛點、輸入格式、輸出格式、驗收標準）。

3. **「特工出規格，Antigravity 落地核心」移交協議 (@Antigravity_Handoff)**：
   - 特工負責業務場景與格式定義，不直接在聊天室內假想編譯 Rust 代碼。
   - 當定義好規格後，特工必須在輸出末尾附帶標準移交標籤區塊，供主理人直接指派後台 Antigravity 落地：
   ```antigravity_handoff
   [MODULE]: 欲實作的模組或工具名稱 (例如 comm-quality-guard)
   [MVP_TARGET]: 本次 MVP 欲解決的核心問題 (例如 消除 Fizz 食字與角括號跳脫)
   [INPUT_SPEC]: 輸入型別與格式範例
   [OUTPUT_SPEC]: 輸出型別與格式範例
   [ACCEPTANCE]: 驗收條件與測試用例
   ```
"""

# 對所有特工實例注入
updated_count = 0
agents_list = data if isinstance(data, list) else data.get('agents', [])

for agent in agents_list:
    name = agent.get('name', '')
    sys_prompt = agent.get('system_prompt', '') or agent.get('prompt', '')
    
    # 檢查是否已注入
    if "特工治理三原則與系統真實底層清冊" not in sys_prompt:
        new_prompt = sys_prompt + GOVERNANCE_AND_CAPABILITY_PROMPT
        if 'system_prompt' in agent:
            agent['system_prompt'] = new_prompt
        else:
            agent['prompt'] = new_prompt
        updated_count += 1
        print(f"Injected into agent: {name} (ID: {agent.get('id')})")

with open(path, 'w', encoding='utf-8') as f:
    json.dump(data, f, ensure_ascii=False, indent=2)

print(f"Successfully updated {updated_count} agents in managed-agents.json!")
