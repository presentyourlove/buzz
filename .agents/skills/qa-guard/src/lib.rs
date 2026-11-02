//! QA-Guard Skill - Rust 內建技能實現
//! 
//! 本技能負責執行 Git Patch 自動語法、編碼與執行預檢，提升部署品質
//! 
//! 核心功能模組：
//! 1. patch_parser.rs - Git Patch 解析器
//! 2. encoding_validator.rs - 編碼驗證器
//! 3. syntax_checker.rs - 語法檢查器
//! 4. cargo_check_runner.rs - Cargo Check 執行器
//! 5. report_generator.rs - 報告生成器
//! 
//! 技能整合點：
//! - Session 建立時發現 (buzz-agent/src/hints.rs)
//! - 載入時機 (buzz-agent/src/builtin.rs:52-124)
//! - Hook 整合 (hooks/mod.rs:63-74)

pub mod patch_parser;
pub mod encoding_validator;
pub mod syntax_checker;
pub mod cargo_check_runner;
pub mod report_generator;

/// 重新導出常用類型和結構，方便技能實現
pub use patch_parser::{ParsedPatch, ParseError};
pub use encoding_validator::{EncodingValidator, EncodingReport, DetectedEncoding, LineEnding, EncodingError, FixError};
pub use syntax_checker::{SyntaxChecker, SyntaxReport, Language, SyntaxError, SyntaxWarning};
pub use cargo_check_runner::{CargoCheckRunner, CargoCheckReport, CargoError, CargoWarning, ErrorLevel};
pub use report_generator::{QaGuardReport, QaGuardPatchResult};

/// 技能入口函數 - 初始化 QA-Guard 技能
/// 
/// # 返回值
/// 
/// 返回配置完成的 QaGuardSkill 實例
/// 
/// # 示例
/// 
/// ```rust
/// use qa_guard::{init_skill, load_config};
/// 
/// #[tokio::main]
/// async fn main() {
///     let skill = init_skill();
///     let config = load_config();
///     
///     // 使用技能執行 QA-Guard 檢查
///     let patch_content = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-fn main() {\n+async fn main() {\n     println!("Hello");\n }\n";
///     
///     match skill.check_patch(patch_content).await {
///         Ok(report) => println!("檢查報告:\n{}", report.generate_markdown()),
///         Err(e) => eprintln!("檢查失敗: {:?}", e),
///     }
/// }
/// ```
pub fn init_skill() -> QaGuardSkill {
    QaGuardSkill::new()
}

/// 載入技能配置
/// 
/// # 返回值
/// 
/// 返回默認的 QA-Guard 技能配置
/// 
/// # 示例
/// 
/// ```rust
/// use qa_guard::load_config;
/// 
/// fn main() {
///     let config = load_config();
///     println!("技能配置：{:?}", config);
/// }
/// ```
pub fn load_config() -> QaGuardConfig {
    QaGuardConfig::default()
}

/// QA-Guard 技能主類
/// 
/// 用於管理 QA-Guard 技能的整體功能
pub struct QaGuardSkill {
    /// 編碼驗證器
    encoding_validator: EncodingValidator,
    /// Patch 解析器
    patch_parser: patch_parser::PatchParser,
    /// 語法檢查器
    syntax_checker: SyntaxChecker,
    /// Cargo Check 執行器
    cargo_runner: CargoCheckRunner,
    /// 報告生成器
    report_generator: report_generator::QaGuardReport,
}

impl QaGuardSkill {
    /// 創建新的技能實例
    /// 
    /// # 返回值
    /// 
    /// 返回新的 QaGuardSkill 實例
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::QaGuardSkill;
    /// 
    /// fn main() {
    ///     let skill = QaGuardSkill::new();
    ///     println!("創建了 QA-Guard 技能");
    /// }
    /// ```
    pub fn new() -> Self {
        let encoding_validator = EncodingValidator::new();
        let patch_parser = patch_parser::PatchParser;
        let syntax_checker = SyntaxChecker::new();
        let cargo_runner = CargoCheckRunner::new(PathBuf::from("."));
        let report_generator = report_generator::QaGuardReport::new(
            Utc::now(),
            report_generator::QaGuardPatchResult::new(0),
            Vec::new(),
            Vec::new(),
            None,
        );
        
        Self {
            encoding_validator,
            patch_parser,
            syntax_checker,
            cargo_runner,
            report_generator,
        }
    }
    
    /// 執行 Patch 檢查
    /// 
    /// # 參數
    /// 
    /// * `patch_content` - Patch 內容
    /// 
    /// # 返回值
    /// 
    /// 返回包含檢查結果的 QaGuardReport，如果檢查失敗，則返回錯誤
    /// 
    /// # 檢查邏輯
    /// 
    /// 1. **解析 Patch** - 使用 Patch 解析器解析 Patch 內容
    /// 2. **編碼檢查** - 使用編碼驗證器檢查編碼和換行符
    /// 3. **語法檢查** - 使用語法檢查器檢查語法
    /// 4. **Cargo Check** - 使用 Cargo Check 執行器執行 Cargo Check
    /// 5. **生成報告** - 使用報告生成器生成檢查報告
    /// 
    /// # 錯誤
    /// 
    /// - `ParseError` - Patch 解析錯誤
    /// - `EncodingError` - 編碼驗證錯誤
    /// - `SyntaxError` - 語法檢查錯誤
    /// - `CargoError` - Cargo Check 錯誤
    /// - `Other` - 其他錯誤
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::QaGuardSkill;
    /// use tokio;
    /// 
    /// #[tokio::main]
    /// async fn main() {
    ///     let skill = QaGuardSkill::new();
    ///     
    ///     let patch_content = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-fn main() {\n+async fn main() {\n     println!("Hello");\n }\n";
    ///     
    ///     match skill.check_patch(patch_content).await {
    ///         Ok(report) => println!("檢查報告:\n{}", report.generate_markdown()),
    ///         Err(e) => eprintln!("檢查失敗: {:?}", e),
    ///     }
    /// }
    /// ```
    pub async fn check_patch(&mut self, patch_content: &str) -> Result<QaGuardReport, QaGuardError> {
        // 1. 解析 Patch
        let parsed_patch = ParsedPatch::parse(patch_content).map_err(QaGuardError::ParseError)?;
        
        // 2. 編碼檢查
        let mut encoding_reports = Vec::new();
        for file in &parsed_patch.files {
            // 讀取檔案內容
            let content = match std::fs::read(&file.old_path) {
                Ok(content) => content,
                Err(_) => continue, // 跳過無法讀取的檔案
            };
            
            let report = self.encoding_validator.validate(&content, &file.old_path);
            encoding_reports.push(report);
        }
        
        // 3. 語法檢查
        let mut syntax_reports = Vec::new();
        for file in &parsed_patch.files {
            if file.is_rust_file() {
                let report = self.syntax_checker.check_file(&file.old_path);
                syntax_reports.push(report);
            }
        }
        
        // 4. Cargo Check
        let cargo_report = if parsed_patch.get_rust_files().is_empty() {
            None
        } else {
            Some(self.cargo_runner.check_incremental(&parsed_patch.get_rust_files().iter().map(|f| f.old_path.clone()).collect()).await.map_err(QaGuardError::CargoError)?)
        };
        
        // 5. 生成報告
        let mut report = QaGuardReport::new(
            Utc::now(),
            report_generator::QaGuardPatchResult::new(parsed_patch.files.len()),
            encoding_reports,
            syntax_reports,
            cargo_report,
        );
        
        Ok(report)
    }
    
    /// 執行完整檢查
    /// 
    /// # 返回值
    /// 
    /// 返回包含檢查結果的 QaGuardReport，如果檢查失敗，則返回錯誤
    /// 
    /// # 檢查邏輯
    /// 
    /// 1. **執行完整 Cargo Check** - 檢查整個專案
    /// 2. **編碼檢查** - 檢查所有檔案的編碼和換行符
    /// 3. **語法檢查** - 檢查所有檔案的語法
    /// 4. **生成報告** - 生成檢查報告
    /// 
    /// # 錯誤
    /// 
    /// - `CargoError` - Cargo Check 錯誤
    /// - `Other` - 其他錯誤
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::QaGuardSkill;
    /// use tokio;
    /// 
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut skill = QaGuardSkill::new();
    ///     
    ///     match skill.check_full().await {
    ///         Ok(report) => println!("完整檢查報告:\n{}", report.generate_markdown()),
    ///         Err(e) => eprintln!("完整檢查失敗: {:?}", e),
    ///     }
    /// }
    /// ```
    pub async fn check_full(&mut self) -> Result<QaGuardReport, QaGuardError> {
        // 1. 執行完整 Cargo Check
        let cargo_report = Some(self.cargo_runner.check_full().await.map_err(QaGuardError::CargoError)?);
        
        // 2. 編碼檢查 - 遍歷專案中的所有 Rust 檔案
        let mut encoding_reports = Vec::new();
        let rust_files = self.find_rust_files();
        for file_path in rust_files {
            // 讀取檔案內容
            let content = match std::fs::read(&file_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            
            let report = self.encoding_validator.validate(&content, &file_path);
            encoding_reports.push(report);
        }
        
        // 3. 語法檢查
        let mut syntax_reports = Vec::new();
        for file_path in &rust_files {
            let report = self.syntax_checker.check_file(file_path);
            syntax_reports.push(report);
        }
        
      
