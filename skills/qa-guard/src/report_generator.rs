//! 報告生成器模組
//! 
//! 負責生成 QA-Guard 檢查報告，支援 Markdown 和 JSON 輸出格式
//! 
//! 主要功能包括：
//! 1. 構建 Markdown 報告
//! 2. 構建 JSON 報告
//! 3. 支援多種輸出格式
//! 4. 支援詳情展開和收起
//! 5. 支援搜尋和篩選
//! 
//! # 使用範例
//! 
//! ```rust
//! use qa_guard::report_generator::{generate_report, QaGuardReport};
//! use qa_guard::patch_parser::ParsedPatch;
//! use qa_guard::encoding_validator::EncodingReport;
//! use qa_guard::syntax_checker::SyntaxReport;
//! use qa_guard::cargo_check_runner::CargoCheckReport;
//! 
//! fn main() {
//!     // 創建測試數據
//!     let patch = ParsedPatch::new(
//!         vec![],
//!         false,
//!         0,
//!         0,
//!         "".to_string(),
//!     );
//!     let encoding_reports = vec![];
//!     let syntax_reports = vec![];
//!     let cargo_report = Some(CargoCheckReport::new(
//!         true,
//!         vec![],
//!         vec![],
//!         1000,
//!     ));
//!     
//!     // 生成報告
//!     let report = generate_report(&patch, &encoding_reports, &syntax_reports, &cargo_report);
//!     
//!     // 生成 Markdown 報告
//!     let md_report = report.generate_markdown();
//!     println!("Markdown 報告:\n{}", md_report);
//!     
//!     // 生成 JSON 報告
//!     let json_report = report.generate_json();
//!     println!("JSON 報告:\n{}", json_report);
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

use crate::{
    patch_parser::ParsedPatch,
    encoding_validator::{EncodingReport, EncodingError},
    syntax_checker::SyntaxReport,
    cargo_check_runner::CargoCheckReport,
};

/// 取得編碼錯誤的簡短分類標籤
fn get_error_line(error: &EncodingError) -> &'static str {
    match error {
        EncodingError::InvalidEncoding(_) => "ENCODING",
        EncodingError::BomFound => "BOM",
        EncodingError::InvalidLineEnding(_) => "LINE_ENDING",
    }
}

/// 組合各項檢查結果生成 QA-Guard 報告
pub fn generate_report(
    patch: &ParsedPatch,
    encoding_reports: &[EncodingReport],
    syntax_reports: &[SyntaxReport],
    cargo_report: &Option<CargoCheckReport>,
) -> QaGuardReport {
    let mut patch_res = QaGuardPatchResult::new(patch.files.len());
    patch_res.total_additions = patch.total_additions;
    patch_res.total_deletions = patch.total_deletions;
    QaGuardReport::new(
        Utc::now(),
        patch_res,
        encoding_reports.to_vec(),
        syntax_reports.to_vec(),
        cargo_report.clone(),
    )
}

/// QA-Guard 報告
/// 
/// 代表 QA-Guard 檢查結果的報告，包含檢查狀態、檢查結果和統計信息
/// 
/// # 欄位
/// 
/// - `timestamp` - 檢查時間
/// - `patch` - Patch 檢查結果
/// - `encoding_reports` - 編碼檢查結果
/// - `syntax_reports` - 語法檢查結果
/// - `cargo_report` - Cargo Check 結果
/// 
/// # 示例
/// 
/// ```rust
/// use qa_guard::report_generator::QaGuardReport;
/// 
/// fn main() {
///     let report = QaGuardReport::new(
///         chrono::Utc::now(),
///         None,
///         vec![],
///         vec![],
///         None,
///     );
///     println!("創建了 QA-Guard 報告: {:?}", report);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaGuardReport {
    /// 檢查時間
    pub timestamp: DateTime<Utc>,
    /// Patch 檢查結果
    pub patch: QaGuardPatchResult,
    /// 編碼檢查結果
    pub encoding_reports: Vec<EncodingReport>,
    /// 語法檢查結果
    pub syntax_reports: Vec<SyntaxReport>,
    /// Cargo Check 結果
    pub cargo_report: Option<CargoCheckReport>,
}

impl QaGuardReport {
    /// 創建新的 QA-Guard 報告
    /// 
    /// # 參數
    /// 
    /// * `timestamp` - 檢查時間
    /// * `patch` - Patch 檢查結果
    /// * `encoding_reports` - 編碼檢查結果
    /// * `syntax_reports` - 語法檢查結果
    /// * `cargo_report` - Cargo Check 結果
    /// 
    /// # 返回值
    /// 
    /// 返回新的 QA-Guard 報告
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::report_generator::QaGuardReport;
    /// use chrono::Utc;
    /// use qa_guard::qa_guard::QaGuardPatchResult;
    /// 
    /// fn main() {
    ///     let report = QaGuardReport::new(
    ///         Utc::now(),
    ///         QaGuardPatchResult::new(1),
    ///         vec![],
    ///         vec![],
    ///         None,
    ///     );
    ///     println!("創建了 QA-Guard 報告: {:?}", report);
    /// }
    /// ```
    pub fn new(
        timestamp: DateTime<Utc>,
        patch: QaGuardPatchResult,
        encoding_reports: Vec<EncodingReport>,
        syntax_reports: Vec<SyntaxReport>,
        cargo_report: Option<CargoCheckReport>,
    ) -> Self {
        Self {
            timestamp,
            patch,
            encoding_reports,
            syntax_reports,
            cargo_report,
        }
    }
    
    /// 生成 Markdown 報告
    /// 
    /// # 返回值
    /// 
    /// 返回 Markdown 格式的報告字符串
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::report_generator::QaGuardReport;
    /// 
    /// fn main() {
    ///     let report = QaGuardReport::new(
    ///         chrono::Utc::now(),
    ///         QaGuardPatchResult::new(1),
    ///         vec![],
    ///         vec![],
    ///         None,
    ///     );
    ///     let md_report = report.generate_markdown();
    ///     println!("Markdown 報告:\n{}", md_report);
    /// }
    /// ```
    pub fn generate_markdown(&self) -> String {
        let mut md = String::new();
        
        // 添加標題
        md.push_str("# QA-Guard 檢查報告\n\n");
        
        // 添加元數據
        md.push_str(&format!("> **檢查時間**：{}  \n", self.timestamp.format("%Y-%m-%d %H:%M:%S")));
        md.push_str(&format!("> **Patch 檔案**：{} 個  \n", self.patch.files_count));
        md.push_str(&format!("> **新增/刪除行**：+{} / -{}  \n\n", self.patch.total_additions, self.patch.total_deletions));
        
        // 添加編碼檢查結果
        if !self.encoding_reports.is_empty() {
            md.push_str("## 編碼與換行符檢查\n\n");
            for report in &self.encoding_reports {
                let status = if report.is_valid { "✅ 通過" } else { "❌ 失敗" };
                md.push_str(&format!("- **{}** `{} ({:?}, {:?})`\n", status, report.file_path.display(), report.encoding, report.line_ending));
                for error in &report.errors {
                    md.push_str(&format!("  - `{}`: {}\n", get_error_line(error), error));
                }
            }
            md.push_str("\n");
        }
        
        // 添加語法檢查結果
        if !self.syntax_reports.is_empty() {
            md.push_str("## 語法檢查\n\n");
            for report in &self.syntax_reports {
                let status = if report.is_valid { "✅ 通過" } else { "❌ 失敗" };
                md.push_str(&format!("- **{}** `{} ({:?})`\n", status, report.file_path.display(), report.language));
                for error in &report.errors {
                    md.push_str(&format!("  - 第 {} 行: {}\n", error.line, error.message));
                }
                for warning in &report.warnings {
                    md.push_str(&format!("  - 第 {} 行: 警告: {}\n", warning.line, warning.message));
                }
            }
            md.push_str("\n");
        }
        
        // 添加 Cargo Check 結果
        if let Some(cargo_report) = &self.cargo_report {
            md.push_str("## Cargo Check (Rust)\n\n");
            md.push_str(&format!("- **{}** (耗時 {}ms)\n", if cargo_report.success { "✅ 通過" } else { "❌ 失敗" }, cargo_report.duration_ms));
            for error in &cargo_report.errors {
                md.push_str(&format!("  - `{}：{}：{}`: [{}] {}\n", error.file.display(), error.line, error.column, error.code.as_deref().unwrap_or(""), error.message));
                for note in &error.notes {
                    md.push_str(&format!("    - 備註: {}\n", note));
                }
            }
            for warning in &cargo_report.warnings {
                md.push_str(&format!("  - `{}：{}：{}`: [{}] 警告: {}\n", warning.file.display(), warning.line, warning.column, warning.code.as_deref().unwrap_or(""), warning.message));
                for note in &warning.notes {
                    md.push_str(&format!("    - 備註: {}\n", note));
                }
            }
        }
        
        md
    }
    
    /// 生成 JSON 報告
    /// 
    /// # 返回值
    /// 
    /// 返回 JSON 格式的報告字符串
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::report_generator::QaGuardReport;
    /// 
    /// fn main() {
    ///     let report = QaGuardReport::new(
    ///         chrono::Utc::now(),
    ///         QaGuardPatchResult::new(1),
    ///         vec![],
    ///         vec![],
    ///         None,
    ///     );
    ///     let json_report = report.generate_json();
    ///     println!("JSON 報告:\n{}", json_report);
    /// }
    /// ```
    pub fn generate_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// QA-Guard Patch 結果
/// 
/// 表示 Patch 檢查結果的信息
/// 
/// # 欄位
/// 
/// - `files_count` - Patch 中的檔案數量
/// - `total_additions` - 新增的總行數
/// - `total_deletions` - 刪除的總行數
/// 
/// # 示例
/// 
/// ```rust
/// use qa_guard::report_generator::QaGuardPatchResult;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaGuardPatchResult {
    pub files_count: usize,
    pub total_additions: usize,
    pub total_deletions: usize,
}

impl QaGuardPatchResult {
    pub fn new(files_count: usize) -> Self {
        Self {
            files_count,
            total_additions: 0,
            total_deletions: 0,
        }
    }
}
