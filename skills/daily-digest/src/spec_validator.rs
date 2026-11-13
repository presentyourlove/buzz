//! Spec Validator 模組 (MVP 規範驗證器)
//!
//! 負責在 Vault 同步前驗證生成的每日摘要 Markdown 是否符合 MVP 規格要求。
//!
//! 核心檢查項目：
//! 1. Front-matter 存在性（--- 分隔符）
//! 2. title 欄位格式（YYYY-MM-DD 每日對話摘要與決策複盤）
//! 3. type 欄位（必須為 daily-digest）
//! 4. agent 欄位（必須為 Daily Digest Skill）
//! 5. date 欄位與檔案名稱匹配性
//! 6. tags 陣列（至少包含 digest, daily）
//! 7. threads_count 與 messages_count 整數檢查
//! 8. DoD 檢查清單區塊是否存在

use regex::Regex;

/// 規格驗證錯誤枚舉
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// 缺失 Front-matter (缺少開頭或結尾 ---)
    MissingFrontmatter,
    /// 缺失必填欄位
    MissingField(String),
    /// 欄位值不符合規格要求
    InvalidFieldValue {
        field: String,
        expected: String,
        actual: String,
    },
    /// 日期與檔案名稱不匹配
    DateMismatch {
        frontmatter: String,
        filename: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MissingFrontmatter => write!(f, "缺失 Front-matter 分隔符 (---)"),
            ValidationError::MissingField(field) => write!(f, "缺失必填欄位: {}", field),
            ValidationError::InvalidFieldValue { field, expected, actual } => {
                write!(f, "欄位 {} 格式無效，預期 '{}'，實際 '{}'", field, expected, actual)
            }
            ValidationError::DateMismatch { frontmatter, filename } => {
                write!(f, "Front-matter 日期 '{}' 與檔案名稱 '{}' 不匹配", frontmatter, filename)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// 規格驗證警告枚舉
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationWarning {
    /// 缺少建議欄位
    MissingRecommendedField(String),
    /// 缺少 DoD 檢查清單區塊
    DodChecklistMissing,
    /// 缺少建議標籤
    TagMissing(String),
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationWarning::MissingRecommendedField(field) => write!(f, "建議包含欄位: {}", field),
            ValidationWarning::DodChecklistMissing => write!(f, "建議包含 '## DoD 檢查清單' 區塊"),
            ValidationWarning::TagMissing(tag) => write!(f, "標籤列表中建議包含 '{}'", tag),
        }
    }
}

/// 驗證結果結構體
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// 驗證是否通過（在 strict_mode 下若有任何 error 則為 false）
    pub valid: bool,
    /// 嚴重錯誤列表
    pub errors: Vec<ValidationError>,
    /// 警告列表
    pub warnings: Vec<ValidationWarning>,
}

/// MVP 規格驗證器
pub struct SpecValidator {
    /// 是否啟用嚴格模式
    strict_mode: bool,
    title_regex: Regex,
    date_regex: Regex,
}

impl SpecValidator {
    /// 建立新的 SpecValidator 實例
    pub fn new(strict_mode: bool) -> Self {
        Self {
            strict_mode,
            title_regex: Regex::new(r"^\d{4}-\d{2}-\d{2} 每日對話摘要與決策複盤$").unwrap(),
            date_regex: Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap(),
        }
    }

    /// 驗證 Markdown 內容是否符合規範
    pub fn validate(&self, markdown: &str, filename: &str) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        let clean_content = markdown.trim_start();
        
        // 1. Front-matter 分隔符檢查
        if !clean_content.starts_with("---") {
            if self.strict_mode {
                errors.push(ValidationError::MissingFrontmatter);
            } else {
                warnings.push(ValidationWarning::MissingRecommendedField("frontmatter".to_string()));
            }
            return ValidationResult {
                valid: !self.strict_mode,
                errors,
                warnings,
            };
        }

        // 尋找關閉的 ---
        let after_first = &clean_content[3..];
        let Some(end_idx) = after_first.find("\n---") else {
            if self.strict_mode {
                errors.push(ValidationError::MissingFrontmatter);
            } else {
                warnings.push(ValidationWarning::MissingRecommendedField("frontmatter closing ---".to_string()));
            }
            return ValidationResult {
                valid: !self.strict_mode,
                errors,
                warnings,
            };
        };

        let frontmatter_content = &after_first[..end_idx];
        let body_content = &after_first[end_idx + 4..];

        // 2. 解析 Front-matter 鍵值對
        let mut title_opt: Option<String> = None;
        let mut type_opt: Option<String> = None;
        let mut agent_opt: Option<String> = None;
        let mut date_opt: Option<String> = None;
        let mut tags_opt: Option<Vec<String>> = None;
        let mut threads_count_opt: Option<i64> = None;
        let mut messages_count_opt: Option<i64> = None;

        for line in frontmatter_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let mut val = val.trim().to_string();
                
                // 去除引號
                if (val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\'')) {
                    if val.len() >= 2 {
                        val = val[1..val.len() - 1].to_string();
                    }
                }

                match key {
                    "title" => title_opt = Some(val),
                    "type" => type_opt = Some(val),
                    "agent" => agent_opt = Some(val),
                    "date" => date_opt = Some(val),
                    "threads_count" => threads_count_opt = val.parse::<i64>().ok(),
                    "messages_count" => messages_count_opt = val.parse::<i64>().ok(),
                    "tags" => {
                        // 解析 [tag1, tag2]
                        let v = val.trim_start_matches('[').trim_end_matches(']');
                        let parsed_tags = v.split(',')
                            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<String>>();
                        tags_opt = Some(parsed_tags);
                    }
                    _ => {}
                }
            }
        }

        // 3. 驗證必填欄位 (Title)
        match title_opt {
            Some(title) => {
                if !self.title_regex.is_match(&title) {
                    errors.push(ValidationError::InvalidFieldValue {
                        field: "title".to_string(),
                        expected: "YYYY-MM-DD 每日對話摘要與決策複盤".to_string(),
                        actual: title,
                    });
                }
            }
            None => errors.push(ValidationError::MissingField("title".to_string())),
        }

        // 4. 驗證 type 欄位
        match type_opt {
            Some(t) => {
                if t != "daily-digest" {
                    errors.push(ValidationError::InvalidFieldValue {
                        field: "type".to_string(),
                        expected: "daily-digest".to_string(),
                        actual: t,
                    });
                }
            }
            None => errors.push(ValidationError::MissingField("type".to_string())),
        }

        // 5. 驗證 agent 欄位
        match agent_opt {
            Some(a) => {
                if a != "Daily Digest Skill" {
                    errors.push(ValidationError::InvalidFieldValue {
                        field: "agent".to_string(),
                        expected: "Daily Digest Skill".to_string(),
                        actual: a,
                    });
                }
            }
            None => errors.push(ValidationError::MissingField("agent".to_string())),
        }

        // 6. 驗證 date 欄位與檔案名稱匹配
        match date_opt {
            Some(d) => {
                if !self.date_regex.is_match(&d) {
                    errors.push(ValidationError::InvalidFieldValue {
                        field: "date".to_string(),
                        expected: "YYYY-MM-DD".to_string(),
                        actual: d.clone(),
                    });
                }

                // 檢查檔案名
                let expected_base = filename.trim_end_matches(".md");
                let file_stem = std::path::Path::new(expected_base)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(expected_base);

                if !file_stem.is_empty() && file_stem != d {
                    if self.strict_mode {
                        errors.push(ValidationError::DateMismatch {
                            frontmatter: d,
                            filename: filename.to_string(),
                        });
                    } else {
                        warnings.push(ValidationWarning::MissingRecommendedField(format!(
                            "DateMismatch: frontmatter date '{}' != filename '{}'",
                            d, filename
                        )));
                    }
                }
            }
            None => errors.push(ValidationError::MissingField("date".to_string())),
        }

        // 7. 驗證建議欄位 (threads_count / messages_count)
        if threads_count_opt.is_none() {
            warnings.push(ValidationWarning::MissingRecommendedField("threads_count".to_string()));
        }
        if messages_count_opt.is_none() {
            warnings.push(ValidationWarning::MissingRecommendedField("messages_count".to_string()));
        }

        // 8. 驗證 tags
        match tags_opt {
            Some(tags) => {
                if !tags.iter().any(|t| t == "digest") {
                    warnings.push(ValidationWarning::TagMissing("digest".to_string()));
                }
                if !tags.iter().any(|t| t == "daily") {
                    warnings.push(ValidationWarning::TagMissing("daily".to_string()));
                }
            }
            None => {
                warnings.push(ValidationWarning::MissingRecommendedField("tags".to_string()));
            }
        }

        // 9. 驗證 DoD 檢查清單
        if !body_content.contains("DoD 檢查清單") && !body_content.contains("DoD Checklist") {
            warnings.push(ValidationWarning::DodChecklistMissing);
        }

        let valid = if self.strict_mode {
            errors.is_empty()
        } else {
            true
        };

        ValidationResult {
            valid,
            errors,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_markdown_spec() {
        let validator = SpecValidator::new(true);
        let markdown = r#"---
title: "2026-11-06 每日對話摘要與決策複盤"
type: "daily-digest"
agent: "Daily Digest Skill"
date: "2026-11-06"
threads_count: 5
messages_count: 23
tags: ["digest", "daily"]
---

# 2026-11-06 每日對話摘要與決策複盤

## DoD 檢查清單
- [ ] 決策提煉
- [ ] 執行緒分類
"#;

        let res = validator.validate(markdown, "2026-11-06.md");
        assert!(res.valid, "Expected valid, got errors: {:?}", res.errors);
        assert!(res.errors.is_empty());
        assert!(res.warnings.is_empty());
    }

    #[test]
    fn test_missing_frontmatter() {
        let validator = SpecValidator::new(true);
        let markdown = "# No Frontmatter content";
        let res = validator.validate(markdown, "2026-11-06.md");
        assert!(!res.valid);
        assert!(res.errors.contains(&ValidationError::MissingFrontmatter));
    }

    #[test]
    fn test_invalid_type_and_agent() {
        let validator = SpecValidator::new(true);
        let markdown = r#"---
title: "2026-11-06 每日對話摘要與決策複盤"
type: "weekly-digest"
agent: "Unknown"
date: "2026-11-06"
---
"#;
        let res = validator.validate(markdown, "2026-11-06.md");
        assert!(!res.valid);
        assert!(res.errors.iter().any(|e| matches!(e, ValidationError::InvalidFieldValue { field, .. } if field == "type")));
        assert!(res.errors.iter().any(|e| matches!(e, ValidationError::InvalidFieldValue { field, .. } if field == "agent")));
    }

    #[test]
    fn test_date_mismatch_strict_mode() {
        let validator = SpecValidator::new(true);
        let markdown = r#"---
title: "2026-11-06 每日對話摘要與決策複盤"
type: "daily-digest"
agent: "Daily Digest Skill"
date: "2026-11-06"
---
## DoD 檢查清單
- [ ] item
"#;
        let res = validator.validate(markdown, "2026-11-07.md");
        assert!(!res.valid);
        assert!(res.errors.iter().any(|e| matches!(e, ValidationError::DateMismatch { .. })));
    }

    #[test]
    fn test_date_mismatch_non_strict_mode() {
        let validator = SpecValidator::new(false);
        let markdown = r#"---
title: "2026-11-06 每日對話摘要與決策複盤"
type: "daily-digest"
agent: "Daily Digest Skill"
date: "2026-11-06"
---
## DoD 檢查清單
- [ ] item
"#;
        let res = validator.validate(markdown, "2026-11-07.md");
        assert!(res.valid);
    }
}

