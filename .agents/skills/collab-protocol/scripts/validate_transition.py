#!/usr/bin/env python3
"""
狀態轉移驗證腳本 - 用於驗證協作協議狀態機轉移的有效性
"""

import os
import re
import sys

def validate_transition_config():
    """驗證 protocol.yaml 配置的有效性"""
    # 支援腳本目錄相對路徑或工作目錄
    candidates = [
        os.path.join(os.path.dirname(__file__), '..', 'protocol.yaml'),
        os.path.join(os.getcwd(), 'protocol.yaml'),
        os.path.join(os.getcwd(), '.agents', 'skills', 'collab-protocol', 'protocol.yaml'),
    ]
    yaml_path = None
    for c in candidates:
        if os.path.exists(c):
            yaml_path = c
            break

    if not yaml_path:
        print("❌ 未找到 protocol.yaml 檔案")
        return False

    print(f"📖 讀取設定檔: {os.path.abspath(yaml_path)}")

    try:
        try:
            import yaml
            with open(yaml_path, 'r', encoding='utf-8') as f:
                config = yaml.safe_load(f)

            required_fields = ['version', 'initial_state', 'states']
            for field in required_fields:
                if field not in config:
                    print(f"❌ 缺少必需欄位: {field}")
                    return False

            states = {state['id'] for state in config['states']}
            if config['initial_state'] not in states:
                print(f"❌ 初始狀態 {config['initial_state']} 不存在於狀態定義中")
                return False

            for state in config['states']:
                for transition in state.get('transitions', []):
                    if transition['target'] not in states:
                        print(f"❌ 狀態 {state['id']} 的轉移目標 {transition['target']} 不存在")
                        return False

            print("✅ 協議配置驗證通過 (透過 PyYAML 解析)")
            return True

        except ImportError:
            # 當 Python 環境無 PyYAML 時，採用輕量化零依賴 AST 解析器
            with open(yaml_path, 'r', encoding='utf-8') as f:
                content = f.read()

            v_match = re.search(r'version:\s*["\']?([^"\'\r\n]+)["\']?', content)
            init_match = re.search(r'initial_state:\s*["\']?([^"\'\r\n]+)["\']?', content)
            states = set(re.findall(r'-\s+id:\s*["\']?([^"\'\r\n]+)["\']?', content))
            targets = set(re.findall(r'target:\s*["\']?([^"\'\r\n]+)["\']?', content))

            if not v_match or not init_match or not states:
                print("❌ 缺少必需欄位或未發現 states 節點")
                return False

            initial_state = init_match.group(1).strip()
            if initial_state not in states:
                print(f"❌ 初始狀態 {initial_state} 不存在於狀態定義中: {states}")
                return False

            missing_targets = targets - states
            if missing_targets:
                print(f"❌ 轉移目標不存在於狀態定義中: {missing_targets}")
                return False

            print(f"✅ 協議配置驗證通過 (零依賴解析引擎)")
            print(f"   - 協議版本: {v_match.group(1).strip()}")
            print(f"   - 初始狀態: {initial_state}")
            print(f"   - 狀態節點總數: {len(states)} 個 ({', '.join(sorted(states))})")
            print(f"   - 轉移目標數: {len(targets)} 個，全部 100% 閉環合法")
            return True

    except Exception as e:
        print(f"❌ 驗證過程中出現錯誤: {e}")
        return False

if __name__ == "__main__":
    print("🔍 正在驗證協作協議狀態機配置...")
    success = validate_transition_config()
    sys.exit(0 if success else 1)
