#!/usr/bin/env python3
"""
單元測試：每日進化腳本 (test_daily_evolution.py)

測試覆蓋：
1. 類別初始化與目錄建立
2. 對話分析與多語主題萃取
3. 雜訊過濾器（過濾標題、報告、代碼塊雜訊）
4. 繁體中文提案生成與雙軌推播格式（全量版 vs 頻道精簡版）
5. 檔案存檔與 UTF-8 編碼
"""

import json
import shutil
import tempfile
import unittest
from pathlib import Path
import sys

# 將腳本目錄加入 sys.path
SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from daily_evolution import DailyEvolution


class TestDailyEvolution(unittest.TestCase):
    """每日進化模組單元測試案例"""

    def setUp(self):
        """建立臨時測試工作區"""
        self.test_dir = tempfile.mkdtemp()
        self.evolution = DailyEvolution(
            channel_id="test-channel-uuid-1234",
            relay_url="wss://test.relay",
            private_key="test-key",
            buzz_cli_path="buzz",
            workspace_root=self.test_dir,
        )
        # 單元測試隔離外部網路大模型呼叫
        self.evolution._call_openrouter_llm = lambda prompt: None

    def tearDown(self):
        """清理臨時工作區"""
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_directory_initialization(self):
        """測試 PLANS/ 與 WORK_LOGS/ 目錄是否自動建立"""
        self.assertTrue((Path(self.test_dir) / "PLANS").exists())
        self.assertTrue((Path(self.test_dir) / "WORK_LOGS").exists())

    def test_noise_filtering_in_conversations(self):
        """測試雜訊過濾器是否有效過濾報告與標題"""
        sample_messages = [
            {"content": "## ✅ Phase 1 Complete: Daily Evolution", "pubkey": "pubkey1"},
            {"content": "每日系統進化提案 - 2026-10-28", "pubkey": "pubkey2"},
            {"content": "【安全與品質驗收通過 (QA PASSED)】", "pubkey": "pubkey3"},
            {"content": "```json\n{\"test\": 1}\n```", "pubkey": "pubkey4"},
            {"content": "系統發現了一個重大 bug，需要修復異常！", "pubkey": "pubkey5"},
            {"content": "我們決定同意採用方案 A，請安排待辦任務實作！", "pubkey": "pubkey6"},
        ]
        work_logs = []
        analysis = self.evolution.analyze_conversations(sample_messages, work_logs)

        # 4 則雜訊應被過濾，僅留下 2 則有效訊息被納入統計與萃取
        self.assertIn("錯誤與異常分析 (bugs/errors)", analysis["topics_discussed"])
        self.assertIn("功能開發與演進 (feature development)", analysis["topics_discussed"])
        self.assertTrue(len(analysis["action_items"]) > 0)
        self.assertTrue(len(analysis["decisions"]) > 0)

        # 確認雜訊字樣未進入待辦事項
        for item in analysis["action_items"]:
            self.assertNotIn("## ✅ Phase", item["description"])
            self.assertNotIn("【安全與品質驗收", item["description"])

    def test_is_closure_or_offtopic(self):
        """測試雜訊與非系統話題過濾：結案報告與股票諮詢須被過濾，真實系統瓶頸應保留"""
        self.assertTrue(
            self.evolution._is_closure_or_offtopic("【Phase 1 完成確認】瀏覽器基建全部交付完成")
        )
        self.assertTrue(
            self.evolution._is_closure_or_offtopic("請問 6855 股票轉倉怎麼辦理比較划算？")
        )
        self.assertFalse(
            self.evolution._is_closure_or_offtopic("buzz-acp 單回合超過 7200 秒 timeout 導致任務中斷")
        )

    def test_synthesize_skill_compositions(self):
        """測試既有技能互相合成配方是否可產出 synthesis 候選"""
        from skill_synthesizer import SkillSynthesizer

        synthesizer = SkillSynthesizer(workspace_root=self.test_dir)
        installed_details = {
            "browser-automation": {"description": "browser"},
            "obsidian-digest-sync": {"description": "obsidian"},
        }
        candidates = synthesizer.synthesize_skill_compositions(installed_details, [])
        matched = [c for c in candidates if c.name == "web-intel-to-obsidian"]
        self.assertEqual(len(matched), 1)
        self.assertEqual(matched[0].kind, "synthesis")
        self.assertIn("browser-automation", matched[0].base_skills)
        self.assertIn("obsidian-digest-sync", matched[0].base_skills)

    def test_proposal_dual_track_generation(self):
        """測試雙軌提案生成：全量備查版 vs 頻道專用精簡版"""
        analysis = {
            "message_count": 50,
            "author_activity": [{"author_prefix": "abc12345", "message_count": 50, "percentage": 100.0}],
            "topics_discussed": ["錯誤與異常分析 (bugs/errors)", "系統發布與部署 (deployment)"],
            "action_items": [{"id": "1.1", "description": "修復某問題", "source_message": "測試來源"}],
            "blockers": [{"id": "2.1", "description": "測試阻礙", "source_message": "測試來源"}],
            "decisions": [{"id": "3.1", "description": "測試決策", "source_message": "測試來源"}],
            "work_logs_summary": [{"file": "test.md", "excerpt": "log excerpt", "modified_at": "2026-10-29T00:00:00"}],
        }

        full_proposal, channel_proposal = self.evolution.generate_proposal(analysis, "2026-10-29")

        # 1. 驗證全量版 (含活動與記憶回顧摘要、元數據 JSON)
        self.assertIn("## 📊 本日活動與知識庫回顧摘要 (Daily & Memory Summary)", full_proposal)
        self.assertIn("## 📎 結構化元數據 (Metadata)", full_proposal)
        self.assertIn("```json", full_proposal)

        # 2. 驗證頻道專用精簡版 (剔除分析雜訊，僅保留建議項目、技能合成與快捷批示)
        self.assertNotIn("## 📊 本日活動與知識庫回顧摘要", channel_proposal)
        self.assertNotIn("## 📎 結構化元數據", channel_proposal)
        self.assertNotIn("```json", channel_proposal)
        self.assertIn("Honey2 每日系統進化速讀卡片", channel_proposal)
        self.assertIn("後台架構進化建議", channel_proposal)
        self.assertIn("特工自主進化建議", channel_proposal)
        self.assertIn("技能產出與互相合成建議", channel_proposal)
        self.assertIn("主理人快捷批示", channel_proposal)
        self.assertIn("同意特工", channel_proposal)
        self.assertIn("交由後台", channel_proposal)
        self.assertIn("同意技能:", channel_proposal)

        # 3. 嚴格防吞字規範驗證：禁止未轉義角括號
        self.assertNotIn("<CHANNEL_UUID>", full_proposal)
        self.assertNotIn("<CHANNEL_UUID>", channel_proposal)
        self.assertNotIn("<項目編號>", channel_proposal)

    def test_save_proposal(self):
        """測試提案檔案存檔功能與 UTF-8 編碼"""
        content = "# 測試提案內容 繁體中文"
        saved_path = self.evolution.save_proposal(content, "2026-10-29")
        self.assertTrue(saved_path.exists())
        self.assertEqual(saved_path.name, "DAILY_EVOLUTION_PROPOSAL_20261029.md")
        self.assertEqual(saved_path.read_text(encoding="utf-8"), content)

    def test_codebase_verifier_symbol_and_anti_hallucination(self):
        """測試 CodebaseVerifier 符號驗證與反幻覺校準"""
        verifier = self.evolution.codebase_verifier
        results = verifier.verify_all_features()
        
        # 驗證關鍵特性是否全數通過
        self.assertTrue(results["three_tier_gate"]["status"])
        self.assertTrue(results["ticket_sandbox"]["status"])
        self.assertTrue(results["unslop_guard"]["status"])
        self.assertTrue(results["system_health_guardian"]["status"])
        self.assertTrue(results["obsidian_digest_sync"]["status"])
        self.assertTrue(results["agent_pairing"]["status"])

        # 測試對話反幻覺校準：當對話中提及已實作特性時，應被校準過濾
        sample_analysis = {
            "action_items": [
                {"id": "1.1", "description": "【Phase 2 驗收回報】三大防爆核心功能架構審查完工", "source_message": "對話"},
                {"id": "1.2", "description": "安排下週伺服器網路例行檢驗", "source_message": "對話"}
            ],
            "blockers": [
                {"id": "2.1", "description": "Phase 2 三檔門禁驗收通過", "source_message": "對話"}
            ]
        }
        
        cal_actions, cal_blockers, implemented = verifier.calibrate_recommendations(sample_analysis)
        
        # 已實作的 1.1 與 2.1 應被剔除
        self.assertEqual(len(cal_actions), 1)
        self.assertEqual(cal_actions[0]["id"], "1.2")
        self.assertEqual(len(cal_blockers), 0)
        self.assertGreaterEqual(len(implemented), 2)
        
        # 驗證提案生成時包含代碼庫真實狀態報告
        sample_analysis["implemented_items"] = implemented
        sample_analysis["action_items"] = cal_actions
        sample_analysis["blockers"] = cal_blockers
        sample_analysis["message_count"] = 10
        sample_analysis["author_activity"] = []
        sample_analysis["topics_discussed"] = []
        sample_analysis["decisions"] = []
        sample_analysis["work_logs_summary"] = []

        full_proposal, channel_proposal = self.evolution.generate_proposal(sample_analysis, "2026-11-08")
        self.assertIn("## 🛠️ 代碼庫真實狀態驗證 (Codebase & Test Verification)", full_proposal)
        self.assertIn("ThreeTierGate", full_proposal)
        self.assertIn("代碼庫健康驗證", channel_proposal)


if __name__ == "__main__":
    unittest.main()

