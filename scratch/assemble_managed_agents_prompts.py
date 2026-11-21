import json
import os
import time
import shutil

repo_dir = r"c:\私人\ai agent\buzz"
managed_path = r'C:\Users\user\AppData\Roaming\xyz.block.buzz.app\agents\managed-agents.json'

ts = int(time.time())
bak_path = f"{managed_path}.bak_phase59_{ts}"
shutil.copyfile(managed_path, bak_path)
print(f"Backed up managed-agents.json to: {bak_path}")

# 1. 讀取全域規範
with open(os.path.join(repo_dir, "docs", "GLOBAL_AGENT_PROMPT.md"), "r", encoding="utf-8") as f:
    global_prompt_content = f.read()

# 2. 讀取專案結構與技能
with open(os.path.join(repo_dir, "docs", "PROJECT_STRUCTURE_AND_SKILLS.md"), "r", encoding="utf-8") as f:
    project_skills_content = f.read()

# 3. 讀取各特工 persona
personas = {}
for name in ["fizz", "honey", "honey2", "pollen", "sentry", "wax"]:
    p_path = os.path.join(repo_dir, "docs", "agent_personas", f"{name}.persona.md")
    with open(p_path, "r", encoding="utf-8") as f:
        p_text = f.read()
        # 提取 markdown body (跳過 frontmatter)
        if p_text.startswith("---"):
            end_idx = p_text.find("---", 3)
            if end_idx != -1:
                body = p_text[end_idx+3:].strip()
            else:
                body = p_text
        else:
            body = p_text
        personas[name.lower()] = body

# 4. 組合統一格式
# [專屬職責] + [全域核心規範] + [專案結構與技能目錄]
SHARED_COMMON_BLOCK = f"""

---
# 【特工每次工作前必看：全域核心規範與專案導航】

{global_prompt_content}

---
{project_skills_content}
"""

with open(managed_path, "r", encoding="utf-8") as f:
    data = json.load(f)

agents_list = data if isinstance(data, list) else data.get("agents", [])

updated_count = 0
for ag in agents_list:
    ag_name = (ag.get("name") or "").lower()
    if ag_name in personas:
        specific_persona = personas[ag_name]
        full_assembled_prompt = f"{specific_persona}\n\n{SHARED_COMMON_BLOCK}".strip()
        
        if "system_prompt" in ag:
            ag["system_prompt"] = full_assembled_prompt
        else:
            ag["prompt"] = full_assembled_prompt
        updated_count += 1
        print(f"Updated agent [{ag.get('name')}] with assembled Prompt ({len(full_assembled_prompt)} chars)")

with open(managed_path, "w", encoding="utf-8") as f:
    json.dump(data, f, ensure_ascii=False, indent=2)

print(f"Successfully assembled and updated {updated_count} agent entries in managed-agents.json!")
