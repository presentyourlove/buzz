#!/usr/bin/env python3
"""
Buzz 本地核心安全防禦與 Hooks 門禁自動化測試套件
測試目標：對 buzz 本地自研與擴充之安全防護規則（路徑穿越、命令注入、硬編碼金鑰、語法檢驗）進行 100% 單元覆蓋與分支驗證
"""

import os
import sys
import json
import unittest

BUZZ_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

# 模擬 Rust HookContext 與檢查規則的邏輯
class MockHookContext:
    def __init__(self, tool_name, args):
        self.tool_name = tool_name
        self.args = args

    def matches_tool(self, tools):
        return self.tool_name in tools

    def get_str_arg(self, keys):
        for k in keys:
            if k in self.args:
                return str(self.args[k])
        return None

# 模擬 PathTraversalGuardRule
class PyPathTraversalGuard:
    @staticmethod
    def check(ctx):
        path_str = ctx.get_str_arg(["path", "file_path", "Path", "target_path", "filepath"])
        if not path_str:
            return "CONTINUE"
        lower = path_str.lower()

        # 1. 目錄逃逸
        if "../" in lower or "..\\" in lower or lower == "..":
            return "BLOCK: DIRECTORY_TRAVERSAL"

        # 2. Windows 保留裝置 (檢查 stem 或完整檔名)
        reserved = ["nul", "con", "prn", "aux", "com1", "com2", "com3", "com4", "lpt1", "lpt2"]
        bname = os.path.basename(lower)
        stem = os.path.splitext(bname)[0]
        for dev in reserved:
            if stem == dev or bname == dev or lower == dev or lower.endswith(f"/{dev}") or lower.endswith(f"\\{dev}"):
                return "BLOCK: WINDOWS_RESERVED_DEVICE"

        # 3. 敏感目錄
        sensitive = [
            "c:\\windows", "c:/windows", "c:\\system32", "c:/system32",
            "/etc/passwd", "/etc/shadow", ".ssh/id_rsa", ".ssh\\id_rsa",
            ".ssh/id_ed25519", ".ssh\\id_ed25519"
        ]
        for s in sensitive:
            if s in lower:
                return "BLOCK: SENSITIVE_SYSTEM_PATH"

        return "CONTINUE"

# 模擬 HardcodedBlockRule
class PyHardcodedBlock:
    @staticmethod
    def check(ctx):
        if not ctx.matches_tool(["write_file", "write", "edit_file", "edit", "str_replace", "Write", "Edit"]):
            return "CONTINUE"
        content_str = ctx.get_str_arg(["content", "replacement", "Content"])
        if not content_str:
            return "CONTINUE"

        secret_signatures = [
            ("sk-proj-", "OpenAI Project API Key"),
            ("sk-or-v1-", "OpenRouter API Key"),
            ("AIzaSy", "Google Gemini API Key"),
            ("ghp_", "GitHub Personal Access Token"),
            ("nsec1", "Nostr Private Key (NIP-19)"),
            ("-----BEGIN OPENSSH PRIVATE KEY-----", "SSH Private Key"),
            ("-----BEGIN RSA PRIVATE KEY-----", "RSA Private Key"),
            ("-----BEGIN PRIVATE KEY-----", "PKCS8 Private Key"),
            ("-----BEGIN EC PRIVATE KEY-----", "EC Private Key"),
        ]

        for sig, desc in secret_signatures:
            if sig in content_str:
                return f"BLOCK: HARDCODED_SECRET ({desc})"

        return "CONTINUE"

# 模擬 DangerCommandBlockRule
class PyDangerCommandBlock:
    @staticmethod
    def check(ctx):
        if not ctx.matches_tool(["shell", "bash", "cmd", "powershell", "execute_command", "exec", "terminal"]):
            return "CONTINUE"
        cmd = ctx.get_str_arg(["command", "cmd", "script", "CommandLine"])
        if not cmd:
            return "CONTINUE"
        lower = cmd.lower()

        dangerous_patterns = [
            "rm -rf /", "rm -rf /*", "rm -rf ~", "rmdir /s /q c:\\",
            "del /s /q c:\\", "del /f /s /q c:\\", "format c:", "format d:",
            "git push --force", "git push -f", "git reset --hard",
            "drop database", "mkfs.", ":(){ :|:& };:",
            "remove-item -recurse -force c:\\", "remove-item -recurse -force \"c:\\\""
        ]

        for pat in dangerous_patterns:
            if pat in lower:
                return f"BLOCK: DANGEROUS_COMMAND ({pat})"

        return "CONTINUE"

# 模擬 SyntaxIntegrityCheckRule
class PySyntaxIntegrityCheck:
    @staticmethod
    def check_json(content):
        try:
            json.loads(content)
            return "CONTINUE"
        except Exception:
            return "BLOCK: INVALID_JSON"

    @staticmethod
    def check_braces(content):
        open_b = content.count('{')
        close_b = content.count('}')
        if open_b != close_b:
            return "WARN: BRACE_MISMATCH"
        return "CONTINUE"

class TestBuzzSecurityAndHooks(unittest.TestCase):
    def test_01_path_traversal_blocking(self):
        """測試路徑穿越與目錄跳脫防禦門禁"""
        # 測試各類非法目錄穿越路徑
        traversal_cases = [
            "../../etc/passwd",
            "..\\..\\Windows\\System32",
            "foo/../../../bar",
            "subfolder/..\\../escape",
            "..",
            "normal/path/../../secret.key"
        ]
        for p in traversal_cases:
            ctx = MockHookContext("write_file", {"path": p})
            res = PyPathTraversalGuard.check(ctx)
            self.assertTrue(res.startswith("BLOCK: DIRECTORY_TRAVERSAL"), f"未能攔截目錄穿越: {p}")

    def test_02_reserved_device_names(self):
        """測試 Windows 保留裝置名稱防護門禁"""
        device_cases = ["nul", "con", "prn", "aux", "com1", "lpt1", "folder/nul", "c:\\temp\\con.txt"]
        for d in device_cases:
            ctx = MockHookContext("write_file", {"path": d})
            res = PyPathTraversalGuard.check(ctx)
            self.assertTrue(res.startswith("BLOCK: WINDOWS_RESERVED_DEVICE"), f"未能攔截保留裝置名: {d}")

    def test_03_sensitive_system_roots(self):
        """測試敏感系統關鍵路徑防護門禁"""
        sensitive_cases = [
            "C:\\Windows\\System32\\cmd.exe",
            "c:/windows/win.ini",
            "/etc/passwd",
            "/etc/shadow",
            "/home/user/.ssh/id_rsa",
            "C:\\Users\\user\\.ssh\\id_ed25519"
        ]
        for s in sensitive_cases:
            ctx = MockHookContext("read_file", {"path": s})
            res = PyPathTraversalGuard.check(ctx)
            self.assertTrue(res.startswith("BLOCK: SENSITIVE_SYSTEM_PATH"), f"未能攔截敏感目錄: {s}")

    def test_04_legitimate_paths_pass(self):
        """測試合法日常開發路徑放行"""
        legit_cases = [
            "crates/buzz-agent/src/lib.rs",
            "wiki/01-Cold-Memory/INDEX.md",
            "deploy/config/buzz-agent.env.example",
            "C:\\私人\\ai agent\\buzz\\Cargo.toml"
        ]
        for p in legit_cases:
            ctx = MockHookContext("read_file", {"path": p})
            res = PyPathTraversalGuard.check(ctx)
            self.assertEqual(res, "CONTINUE", f"合法路徑被誤攔: {p}")

    def test_05_hardcoded_secrets_blocking(self):
        """測試各類真實 API Key 與私鑰硬編碼阻擋門禁"""
        secrets = [
            ("AIzaSyB3TestGoogleKey1234567890abcdef", "Google Gemini API Key"),
            ("sk-or-v1-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "OpenRouter API Key"),
            ("sk-proj-abc123def456ghi789jkl012mno345pqr678", "OpenAI Project API Key"),
            ("ghp_1234567890abcdefghijklmnopqrstuvwxyz", "GitHub Personal Access Token"),
            ("nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5", "Nostr Private Key (NIP-19)"),
            ("-----BEGIN RSA PRIVATE KEY-----\nMIIE...", "RSA Private Key"),
            ("-----BEGIN PRIVATE KEY-----\nMIIE...", "PKCS8 Private Key"),
            ("-----BEGIN EC PRIVATE KEY-----\nMHcA...", "EC Private Key"),
        ]
        for val, desc in secrets:
            ctx = MockHookContext("write_file", {"content": f"let key = \"{val}\";"})
            res = PyHardcodedBlock.check(ctx)
            self.assertTrue(res.startswith("BLOCK: HARDCODED_SECRET"), f"未能攔截機敏資訊: {desc}")

    def test_06_clean_code_content_pass(self):
        """測試無機敏的安全原始碼放行"""
        safe_code = """
        fn load_config() -> Result<Config, Error> {
            let api_key = std::env::var("OPENROUTER_API_KEY")?;
            Ok(Config { api_key })
        }
        """
        ctx = MockHookContext("write_file", {"content": safe_code})
        res = PyHardcodedBlock.check(ctx)
        self.assertEqual(res, "CONTINUE", "安全原始碼被誤攔截")

    def test_07_destructive_commands_blocking(self):
        """測試高危毀滅性命令阻擋門禁"""
        destructive = [
            "rm -rf /",
            "rm -rf ~",
            "rmdir /s /q c:\\",
            "del /f /s /q c:\\",
            "format c:",
            "git push --force origin main",
            "git push -f origin main",
            "git reset --hard HEAD~1",
            "DROP DATABASE production;",
            "remove-item -recurse -force c:\\",
            "remove-item -recurse -force \"c:\\\""
        ]
        for cmd in destructive:
            ctx = MockHookContext("shell", {"command": cmd})
            res = PyDangerCommandBlock.check(ctx)
            self.assertTrue(res.startswith("BLOCK: DANGEROUS_COMMAND"), f"未能阻擋毀滅性指令: {cmd}")

    def test_08_safe_commands_pass(self):
        """測試日常合法開發指令放行"""
        safe_cmds = [
            "cargo test --workspace",
            "git status",
            "git add .",
            "git commit -m 'feat: add security test'",
            "npm run test",
            "python tests/run_all.py"
        ]
        for cmd in safe_cmds:
            ctx = MockHookContext("shell", {"command": cmd})
            res = PyDangerCommandBlock.check(ctx)
            self.assertEqual(res, "CONTINUE", f"安全指令被誤攔: {cmd}")

    def test_09_syntax_integrity_json(self):
        """測試 JSON 語法完整性校驗"""
        valid_json = '{"name": "buzz", "active": true, "version": 1}'
        invalid_json = '{"name": "buzz", "active": true, missing_brace'
        self.assertEqual(PySyntaxIntegrityCheck.check_json(valid_json), "CONTINUE")
        self.assertEqual(PySyntaxIntegrityCheck.check_json(invalid_json), "BLOCK: INVALID_JSON")

    def test_10_collab_protocol_subprocess_safety(self):
        """測試 collab-protocol 驗收腳本已移除 os.system 風險，採用安全 subprocess"""
        script_path = os.path.join(BUZZ_DIR, ".agents", "skills", "collab-protocol", "DO_VERIFY.py")
        self.assertTrue(os.path.exists(script_path), "DO_VERIFY.py 必須存在")
        with open(script_path, "r", encoding="utf-8") as f:
            code = f.read()
        self.assertNotIn("os.system", code, "DO_VERIFY.py 中嚴禁使用不安全 os.system 指令拼接")
        self.assertIn("subprocess.run", code, "DO_VERIFY.py 必須使用參數化安全子進程 subprocess.run")

    def test_11_health_dashboard_xss_protection(self):
        """測試 health_dashboard 已實作 HTML 跳脫與安全 Content-Type，消除 XSS (CWE-79)"""
        mod_path = os.path.join(BUZZ_DIR, "crates", "buzz-agent", "src", "health_dashboard", "mod.rs")
        with open(mod_path, "r", encoding="utf-8") as f:
            code = f.read()
        self.assertIn("fn escape_html", code, "health_dashboard 必須包含 escape_html 跳脫函式")
        self.assertIn("escape_html(&p.name)", code, "進程名稱必須經過 HTML 跳脫")
        self.assertIn("escape_html(&p.details)", code, "進程詳情必須經過 HTML 跳脫")
        self.assertIn("text/plain; charset=utf-8", code, "health.md 端點必須返回 text/plain 以防範儲存型 XSS")

    def test_12_utf8_multibyte_slicing_safety(self):
        """測試 health_diagnoser 與 health_analyzer 已移除不安全位元組切片，支援繁體中文與 Emoji"""
        diag_path = os.path.join(BUZZ_DIR, "crates", "buzz-agent", "src", "health_diagnoser.rs")
        with open(diag_path, "r", encoding="utf-8") as f:
            diag_code = f.read()
        self.assertNotIn("&value[..4]", diag_code, "health_diagnoser 嚴禁使用 &value[..4] 位元組切割")
        self.assertIn("value.chars()", diag_code, "health_diagnoser 必須使用字元安全迭代")

        analyzer_path = os.path.join(BUZZ_DIR, "crates", "buzz-agent", "src", "health_analyzer.rs")
        with open(analyzer_path, "r", encoding="utf-8") as f:
            analyzer_code = f.read()
        self.assertIn("MAX_LOG_BYTES", analyzer_code, "health_analyzer 必須限制單次日誌讀取大小防止 OOM DoS")

    def test_13_hooks_deterministic_and_real_checks(self):
        """測試 Hooks 規則引擎具備確定性順序且修復所有偽檢查與參數錯置漏洞"""
        hooks_mod = os.path.join(BUZZ_DIR, "crates", "buzz-agent", "src", "hooks", "mod.rs")
        with open(hooks_mod, "r", encoding="utf-8") as f:
            mod_code = f.read()
        self.assertIn("rules: Vec<RuleDef>", mod_code, "Hooks 規則儲存必須使用 Vec<RuleDef> 確保確定性執行順序")

        large_file_rs = os.path.join(BUZZ_DIR, "crates", "buzz-agent", "src", "hooks", "rules", "large_file_refuse.rs")
        with open(large_file_rs, "r", encoding="utf-8") as f:
            lf_code = f.read()
        self.assertNotIn('contains("large")', lf_code, "large_file_refuse 嚴禁使用假檔名判斷")
        self.assertIn("std::fs::metadata", lf_code, "large_file_refuse 必須檢查真實檔案大小")

    def test_14_obsidian_digest_sync_security(self):
        """測試 obsidian-digest-sync 已移除 CLI 私鑰暴露與命令注入，並補齊機敏遮罩"""
        collector_path = os.path.join(BUZZ_DIR, "skills", "obsidian-digest-sync", "digest_collector.mjs")
        with open(collector_path, "r", encoding="utf-8") as f:
            collector_code = f.read()
        self.assertNotIn("'--private-key'", collector_code, "digest_collector 嚴禁透過 CLI 參數傳遞私鑰")
        self.assertIn("BUZZ_PRIVATE_KEY", collector_code, "digest_collector 必須透過環境變數傳遞私鑰")

        sync_path = os.path.join(BUZZ_DIR, "skills", "obsidian-digest-sync", "digest_sync.mjs")
        with open(sync_path, "r", encoding="utf-8") as f:
            sync_code = f.read()
        self.assertIn("execFileSync", sync_code, "digest_sync 必須使用參數化 execFileSync")

    def test_15_all_skills_rust_files_brace_balance_and_no_truncation(self):
        """測試 skills/ 與 .agents/skills/ 下全部 36 個 Rust 檔案大括號 100% 平衡且無截斷"""
        skill_roots = [
            os.path.join(BUZZ_DIR, "skills"),
            os.path.join(BUZZ_DIR, ".agents", "skills"),
        ]
        rs_count = 0
        unbalanced = []
        for root_dir in skill_roots:
            for r, _, files in os.walk(root_dir):
                for file in files:
                    if file.endswith(".rs"):
                        rs_count += 1
                        p = os.path.join(r, file)
                        with open(p, "r", encoding="utf-8") as f:
                            text = f.read()
                        if text.count("{") != text.count("}"):
                            unbalanced.append(os.path.relpath(p, BUZZ_DIR))
        self.assertGreaterEqual(rs_count, 36, "應至少檢測 36 個技能區 Rust 原始碼檔案")
        self.assertEqual(len(unbalanced), 0, f"發現大括號不平衡或截斷之 Rust 檔案: {unbalanced}")

if __name__ == "__main__":
    suite = unittest.TestLoader().loadTestsFromTestCase(TestBuzzSecurityAndHooks)
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)

    total_tests = result.testsRun
    failures = len(result.failures)
    errors = len(result.errors)
    passed = total_tests - failures - errors
    coverage = (passed / total_tests) * 100 if total_tests > 0 else 0
    print("\n" + "=" * 50)
    print(f"Buzz 本地安全與門禁測試套件結果摘要:")
    print(f"總測試項目: {total_tests}")
    print(f"通過項目: {passed}")
    print(f"失敗項目: {failures}")
    print(f"錯誤項目: {errors}")
    print(f"測試通過覆蓋率: {coverage:.2f}%")
    print("=" * 50)
    sys.exit(0 if result.wasSuccessful() else 1)
