//! Agent-S 技能動態合成與校驗門禁 (SkillSynthesizer)
//!
//! 從特工成功軌跡中萃取結構化工作流，合成標準 SKILL.md，
//! 並通過 Front-matter 合規性與高危指令防注入安全門禁。

use serde::{Deserialize, Serialize};

/// 技能合成錯誤列舉
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSynthesisError {
    InvalidName(String),
    MissingFrontmatter(String),
    SecurityInjectionDetected(String),
    SizeLimitExceeded { actual: usize, max: usize },
    EmptyBody,
}

impl std::fmt::Display for SkillSynthesisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillSynthesisError::InvalidName(msg) => write!(f, "技能名稱不合規: {msg}"),
            SkillSynthesisError::MissingFrontmatter(msg) => write!(f, "缺失標準 YAML Front-matter: {msg}"),
            SkillSynthesisError::SecurityInjectionDetected(msg) => {
                write!(f, "安全門禁攔截：偵測到危險代碼或高危指令 ({msg})")
            }
            SkillSynthesisError::SizeLimitExceeded { actual, max } => {
                write!(f, "技能檔案超出 Context 防爆上限 (實際 {actual} 位元組，上限 {max} 位元組)")
            }
            SkillSynthesisError::EmptyBody => write!(f, "技能正文內容不可為空"),
        }
    }
}

impl std::error::Error for SkillSynthesisError {}

/// 合成技能實體
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SynthesizedSkill {
    pub name: String,
    pub description: String,
    pub full_skill_md: String,
    pub created_at: u64,
}

/// 技能合成器
#[derive(Debug, Default, Clone)]
pub struct SkillSynthesizer {
    max_size_bytes: usize,
}

impl SkillSynthesizer {
    pub fn new() -> Self {
        Self {
            max_size_bytes: 32 * 1024, // 32KB 防爆上限
        }
    }

    /// 從結構化步驟中動態合成標準 SKILL.md
    pub fn synthesize(
        &self,
        name: &str,
        description: &str,
        steps: &[&str],
        guardrails: &[&str],
        timestamp: u64,
    ) -> Result<SynthesizedSkill, SkillSynthesisError> {
        // 1. 名稱預檢
        if name.trim().is_empty() || name.contains(' ') || name.chars().any(|c| c.is_uppercase()) {
            return Err(SkillSynthesisError::InvalidName(
                "技能名稱必須使用小寫 kebab-case 格式且不可包含空格".into(),
            ));
        }

        if steps.is_empty() {
            return Err(SkillSynthesisError::EmptyBody);
        }

        // 2. 組合 Markdown 內容
        let mut md = String::new();
        md.push_str("---\n");
        md.push_str(&format!("name: {}\n", name));
        md.push_str(&format!("description: {}\n", description));
        md.push_str("---\n\n");

        md.push_str(&format!("# 技能手冊：{}\n\n", name));
        md.push_str(&format!("> **說明**：{}\n\n", description));

        md.push_str("## 🎯 執行步驟指南 (Execution Steps)\n\n");
        for (idx, step) in steps.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", idx + 1, step));
        }

        if !guardrails.is_empty() {
            md.push_str("\n## 🛡️ 安全門禁與守則 (Guardrails)\n\n");
            for guard in guardrails {
                md.push_str(&format!("- [ ] {}\n", guard));
            }
        }

        // 3. 通過安全與格式校驗
        self.validate_skill_md(&md)?;

        Ok(SynthesizedSkill {
            name: name.to_string(),
            description: description.to_string(),
            full_skill_md: md,
            created_at: timestamp,
        })
    }

    /// 驗證任意 SKILL.md 的格式合規性與安全性
    pub fn validate_skill_md(&self, content: &str) -> Result<(), SkillSynthesisError> {
        let bytes_len = content.len();
        if bytes_len > self.max_size_bytes {
            return Err(SkillSynthesisError::SizeLimitExceeded {
                actual: bytes_len,
                max: self.max_size_bytes,
            });
        }

        // 檢查 Front-matter
        if !content.starts_with("---\n") {
            return Err(SkillSynthesisError::MissingFrontmatter(
                "檔案必須以 '---' 開頭".into(),
            ));
        }

        let rest = &content[4..];
        let end_idx = rest.find("\n---\n").ok_or_else(|| {
            SkillSynthesisError::MissingFrontmatter("未找到結束之 '---' 標記".into())
        })?;

        let frontmatter = &rest[..end_idx];
        if !frontmatter.contains("name:") {
            return Err(SkillSynthesisError::MissingFrontmatter("缺少 'name:' 欄位".into()));
        }
        if !frontmatter.contains("description:") {
            return Err(SkillSynthesisError::MissingFrontmatter("缺少 'description:' 欄位".into()));
        }

        // 安全防注入檢查 (黑名單檢驗)
        let prohibited_patterns = [
            "rm -rf /",
            "rm -rf *",
            "del /s /q c:\\",
            "format c:",
            ":(){ :|:& };:",
            "mkfs.",
            "<script>",
            "drop database",
        ];

        let content_lower = content.to_lowercase();
        for pattern in &prohibited_patterns {
            if content_lower.contains(pattern) {
                return Err(SkillSynthesisError::SecurityInjectionDetected(
                    pattern.to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_synthesizer_success() {
        let synth = SkillSynthesizer::new();
        let steps = [
            "讀取本地 Cargo.toml 設定",
            "注入 AWS_LC_SYS_NO_ASM 旗標",
            "執行離線 cargo check 驗證",
        ];
        let guards = [
            "嚴禁連網下載外部未受信任 crates",
            "保留原有依賴版本設定",
        ];

        let skill = synth
            .synthesize(
                "cargo-isolated-build",
                "執行 Windows 隔離相容性構建與檢查",
                &steps,
                &guards,
                1794125000,
            )
            .expect("合成應成功");

        assert_eq!(skill.name, "cargo-isolated-build");
        assert!(skill.full_skill_md.contains("name: cargo-isolated-build"));
        assert!(skill.full_skill_md.contains("## 🎯 執行步驟指南"));
        assert!(skill.full_skill_md.contains("## 🛡️ 安全門禁與守則"));
    }

    #[test]
    fn test_skill_synthesizer_security_rejection() {
        let synth = SkillSynthesizer::new();
        let dangerous_content = "---\nname: dangerous-skill\ndescription: test\n---\n\nrm -rf / --no-preserve-root";
        
        let err = synth.validate_skill_md(dangerous_content).unwrap_err();
        match err {
            SkillSynthesisError::SecurityInjectionDetected(pattern) => {
                assert_eq!(pattern, "rm -rf /");
            }
            _ => panic!("預期攔截危險指令，實際卻回傳: {:?}", err),
        }
    }

    #[test]
    fn test_skill_synthesizer_invalid_name() {
        let synth = SkillSynthesizer::new();
        let err = synth
            .synthesize("Invalid Name", "測試", &["步驟一"], &[], 1794125000)
            .unwrap_err();
        match err {
            SkillSynthesisError::InvalidName(_) => {}
            _ => panic!("預期名稱錯誤，實際卻回傳: {:?}", err),
        }
    }
}

