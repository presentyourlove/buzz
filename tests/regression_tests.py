#!/usr/bin/env python3
"""
回歸測試套件 - 18 項測試案例

覆蓋範圍：
- 對話萃取精準度測試 (5 項)
- 核心決策識別測試 (4 項)
- 雜訊過濾邊界測試 (3 項)
- 事件結構保留測試 (3 項)
- 技能落庫驗證測試 (2 項)
- Git 回滾機制防幻覺驗證測試 (1 項)
"""

import unittest
import json
import tempfile
import os
import sys
import re
from pathlib import Path

# 添加 scripts 目錄到路徑
CURRENT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = CURRENT_DIR.parent
sys.path.insert(0, str(PROJECT_ROOT / 'scripts'))

from extract_conversation import ConversationExtractor, ExtractedMessage


class TestConversationExtraction(unittest.TestCase):
    """對話萃取核心功能測試"""
    
    def setUp(self):
        self.extractor = ConversationExtractor(strict_mode=True)
        self.lenient_extractor = ConversationExtractor(strict_mode=False)
    
    def test_01_filter_markdown_headers(self):
        """測試 1：過濾 Markdown 標題"""
        events = [{
            'id': 'evt1',
            'from': 'user1',
            'time': '2026-10-30T00:00:00Z',
            'content': '# 每日進化報告\n## 子標題\n### 三級標題',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 0)
    
    def test_02_filter_list_items(self):
        """測試 2：過濾普通列表項目"""
        events = [{
            'id': 'evt2',
            'from': 'user2',
            'time': '2026-10-30T00:00:00Z',
            'content': '- 列表項目 1\n* 列表項目 2',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 0)
    
    def test_03_filter_activity_stats(self):
        """測試 3：過濾活動統計文字"""
        events = [{
            'id': 'evt3',
            'from': 'user3',
            'time': '2026-10-30T00:00:00Z',
            'content': '活動統計：今日共有 10 條訊息',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 0)
    
    def test_04_filter_dividers(self):
        """測試 4：過濾分隔線"""
        events = [{
            'id': 'evt4',
            'from': 'user4',
            'time': '2026-10-30T00:00:00Z',
            'content': '---\n===\n--------',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 0)
    
    def test_05_retain_proposal_keyword(self):
        """測試 5：保留「建議進化項目」"""
        events = [{
            'id': 'evt5',
            'from': 'honey2',
            'time': '2026-10-30T00:00:00Z',
            'content': '建議進化項目：優化記憶同步效能',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].message_type, 'proposal')
    
    def test_06_retain_review_options(self):
        """測試 6：保留「審核選項」"""
        events = [{
            'id': 'evt6',
            'from': 'honey2',
            'time': '2026-10-30T00:00:00Z',
            'content': '審核選項：A) 全量實作 B) 最小試驗',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].message_type, 'review_option')
    
    def test_07_retain_authorization(self):
        """測試 7：保留「授權」關鍵字"""
        events = [{
            'id': 'evt7',
            'from': 'owner',
            'time': '2026-10-30T00:00:00Z',
            'content': '授權 Pollen 執行全量實作',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].message_type, 'decision')
    
    def test_08_retain_approval(self):
        """測試 8：保留「同意」與「批准」關鍵字"""
        events = [{
            'id': 'evt8',
            'from': 'owner',
            'time': '2026-10-30T00:00:00Z',
            'content': '同意（全量實作）',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].message_type, 'decision')
    
    def test_09_filter_code_blocks(self):
        """測試 9：過濾代碼塊並保留核心決策"""
        events = [{
            'id': 'evt9',
            'from': 'dev',
            'time': '2026-10-30T00:00:00Z',
            'content': '```python\ndef hello():\n    print("world")\n```\n建議進化項目：新增功能',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        self.assertNotIn("```", result[0].content)
        self.assertNotIn("def hello", result[0].content)
        self.assertIn("建議進化項目", result[0].content)
    
    def test_10_filter_quote_blocks(self):
        """測試 10：過濾引用塊"""
        events = [{
            'id': 'evt10',
            'from': 'user',
            'time': '2026-10-30T00:00:00Z',
            'content': '> 這是引用的內容\n> 多行引用\n建議進化項目：保留核心',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        self.assertNotIn(">", result[0].content)
        self.assertIn("建議進化項目", result[0].content)
    
    def test_11_lenient_mode_retains_short_content(self):
        """測試 11：寬鬆模式保留短內容"""
        events = [{
            'id': 'evt11',
            'from': 'user',
            'time': '2026-10-30T00:00:00Z',
            'content': 'OK',
            'tags': [],
            'parsed': {}
        }]
        strict_result = self.extractor.extract(events)
        lenient_result = self.lenient_extractor.extract(events)
        self.assertEqual(len(strict_result), 0, "嚴格模式應過濾短內容")
        self.assertEqual(len(lenient_result), 1, "寬鬆模式應保留短內容")
    
    def test_12_empty_content_filtered(self):
        """測試 12：空內容被過濾"""
        events = [{
            'id': 'evt12',
            'from': 'user',
            'time': '2026-10-30T00:00:00Z',
            'content': '',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 0)
    
    def test_13_whitespace_only_filtered(self):
        """測試 13：純空白內容被過濾"""
        events = [{
            'id': 'evt13',
            'from': 'user',
            'time': '2026-10-30T00:00:00Z',
            'content': '   \n\t\n  ',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 0)
    
    def test_14_mixed_content_extraction(self):
        """測試 14：混合內容正確萃取核心決策"""
        events = [{
            'id': 'evt14',
            'from': 'mixed',
            'time': '2026-10-30T00:00:00Z',
            'content': '# 標題\n活動統計：100 條\n建議進化項目：核心功能\n```code```\n同意實施',
            'tags': [],
            'parsed': {}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        content = result[0].content
        self.assertNotIn("# 標題", content)
        self.assertNotIn("活動統計", content)
        self.assertNotIn("```", content)
        self.assertIn("建議進化項目", content)
        self.assertIn("同意實施", content)
    
    def test_15_event_structure_preserved(self):
        """測試 15：事件結構完整保留"""
        events = [{
            'id': 'evt15',
            'from': 'tester',
            'time': '2026-10-30T12:34:56Z',
            'content': '建議進化項目：測試結構保留',
            'tags': [['h', 'channel1'], ['p', 'pubkey1']],
            'parsed': {'root': 'root1', 'mentions': ['pollen']}
        }]
        result = self.extractor.extract(events)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].event_id, 'evt15')
        self.assertEqual(result[0].author, 'tester')
        self.assertEqual(result[0].timestamp, '2026-10-30T12:34:56Z')
        self.assertEqual(result[0].content, '建議進化項目：測試結構保留')
        self.assertEqual(result[0].tags, [['h', 'channel1'], ['p', 'pubkey1']])


class TestSkillDeploymentVerification(unittest.TestCase):
    """技能雙軌落庫結構驗證測試"""
    
    def test_16_obsidian_digest_sync_structure(self):
        """測試 16：obsidian-digest-sync 技能結構健全性"""
        for base in [Path('C:/Users/user/.buzz'), PROJECT_ROOT]:
            skill_dir = base / 'skills' / 'obsidian-digest-sync'
            manifest = skill_dir / 'manifest.json'
            main_js = skill_dir / 'main.js'
            self.assertTrue(manifest.exists(), f"manifest.json 應存在於 {skill_dir}")
            self.assertTrue(main_js.exists(), f"main.js 應存在於 {skill_dir}")
            
            with open(manifest, 'r', encoding='utf-8') as f:
                data = json.load(f)
                self.assertEqual(data.get('name'), 'obsidian-digest-sync')
                self.assertIn('version', data)
    
    def test_17_system_health_guardian_structure(self):
        """測試 17：system-health-guardian 技能結構健全性"""
        for base in [Path('C:/Users/user/.buzz'), PROJECT_ROOT]:
            skill_dir = base / 'skills' / 'system-health-guardian'
            manifest = skill_dir / 'manifest.json'
            main_js = skill_dir / 'main.js'
            self.assertTrue(manifest.exists(), f"manifest.json 應存在於 {skill_dir}")
            self.assertTrue(main_js.exists(), f"main.js 應存在於 {skill_dir}")
            
            with open(manifest, 'r', encoding='utf-8') as f:
                data = json.load(f)
                self.assertEqual(data.get('name'), 'system-health-guardian')
                self.assertIn('version', data)


class TestRollbackMechanism(unittest.TestCase):
    """回滾機制防幻覺測試"""
    
    def test_18_plan_v2_rollback_command_validity(self):
        """測試 18：計畫書回滾指令必須為可執行命令，禁止 Git log 輸出"""
        plan_path = Path('C:/Users/user/.buzz/PLANS/DAILY_EVOLUTION_IMPLEMENTATION_PLAN_20261029_v2.md')
        self.assertTrue(plan_path.exists(), "計畫書 v2 應存在")
        
        with open(plan_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 嚴格禁止出現 HEAD is now at 幻覺
        self.assertNotIn("HEAD is now at", content, "計畫書內禁止包含 'HEAD is now at' 幻覺字串！")
        
        # 必須包含標準可執行指令格式
        self.assertRegex(content, r'git -C "[^"]+" reset --hard', "回滾機制必須包含標準 git reset 可執行終端指令")


if __name__ == '__main__':
    unittest.main()

