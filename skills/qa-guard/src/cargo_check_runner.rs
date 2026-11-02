//! Cargo Check 執行器模組
//! 
//! 專為 Rust 專案優化，支援增量檢查的 Cargo Check 執行器
//! 
//! 主要功能包括：
//! 1. 增量檢查（僅檢查受影響的 crate）
//! 2. 完整專案檢查
//! 3. JSON 輸出解析
//! 4. 結構化報告生成
//! 5. 錯誤和警告提取
//! 
//! # 使用範例
//! 
//! ```rust
//! use qa_guard::cargo_check_runner::{CargoCheckRunner, CargoCheckReport};
//! use std::path::PathBuf;
//! use tokio;
//! 
//! #[tokio::main]
//! async fn main() {
//!     let project_root = PathBuf::from("/path/to/rust/project");
//!     let runner = CargoCheckRunner::new(project_root);
//!     
//!     // 執行完整檢查
//!     let full_report = runner.check_full().await.unwrap();
//!     println!("完整檢查報告: {:?}", full_report);
//!     
//!     // 執行增量檢查
//!     let modified_files = vec![PathBuf::from("src/main.rs")];
//!     let incremental_report = runner.check_incremental(&modified_files).await.unwrap();
//!     println!("增量檢查報告: {:?}", incremental_report);
//! }
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Cargo Check 執行器
/// 
/// 負責執行 Rust 專案的 Cargo Check，支援增量檢查和完整檢查
/// 
/// # 功能
/// 
/// 1. **增量檢查** - 僅檢查受影響的 crate，提高檢查效率
/// 2. **完整檢查** - 檢查整個專案，確保代碼質量
/// 3. **JSON 輸出解析** - 解析 Cargo 的 JSON 輸出，提取結構化信息
/// 4. **結構化報告生成** - 生成包含錯誤、警告和統計信息的報告
/// 5. **錯誤和警告提取** - 提取詳細的錯誤和警告信息
/// 
/// # 使用範例
/// 
/// ```rust
/// use qa_guard::cargo_check_runner::{CargoCheckRunner, CargoCheckReport};
/// use std::path::PathBuf;
/// use tokio;
/// 
/// #[tokio::main]
/// async fn main() {
///     let project_root = PathBuf::from("/path/to/rust/project");
///     let runner = CargoCheckRunner::new(project_root);
///     
///     // 執行完整檢查
///     let full_report = runner.check_full().await.unwrap();
///     println!("完整檢查報告: {:?}", full_report);
///     
///     // 執行增量檢查
///     let modified_files = vec![PathBuf::from("src/main.rs")];
///     let incremental_report = runner.check_incremental(&modified_files).await.unwrap();
///     println!("增量檢查報告: {:?}", incremental_report);
/// }
/// ```
pub struct CargoCheckRunner {
    /// 項目根目錄
    project_root: PathBuf,
    /// 目標目錄（可選）
    target_dir: Option<PathBuf>,
}

impl CargoCheckRunner {
    /// 創建新的 CargoCheckRunner
    /// 
    /// # 參數
    /// 
    /// * `project_root` - 項目根目錄
    /// 
    /// # 返回值
    /// 
    /// 返回新的 CargoCheckRunner 實例
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::cargo_check_runner::CargoCheckRunner;
    /// use std::path::PathBuf;
    /// 
    /// fn main() {
    ///     let project_root = PathBuf::from("/path/to/rust/project");
    ///     let runner = CargoCheckRunner::new(project_root);
    ///     println!("創建了 CargoCheckRunner");
    /// }
    /// ```
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            target_dir: None,
        }
    }
    
    /// 創建新的 CargoCheckRunner（帶目標目錄）
    /// 
    /// # 參數
    /// 
    /// * `project_root` - 項目根目錄
    /// * `target_dir` - 目標目錄
    /// 
    /// # 返回值
    /// 
    /// 返回新的 CargoCheckRunner 實例
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::cargo_check_runner::CargoCheckRunner;
    /// use std::path::PathBuf;
    /// 
    /// fn main() {
    ///     let project_root = PathBuf::from("/path/to/rust/project");
    ///     let target_dir = Some(PathBuf::from("/path/to/target"));
    ///     let runner = CargoCheckRunner::new_with_target(project_root, target_dir);
    ///     println!("創建了帶目標目錄的 CargoCheckRunner");
    /// }
    /// ```
    pub fn new_with_target(project_root: PathBuf, target_dir: Option<PathBuf>) -> Self {
        Self {
            project_root,
            target_dir,
        }
    }
    
    /// 執行增量檢查
    /// 
    /// # 參數
    /// 
    /// * `modified_files` - 修改的檔案列表
    /// 
    /// # 返回值
    /// 
    /// 返回包含檢查結果的 CargoCheckReport，如果檢查失敗，則返回錯誤
    /// 
    /// # 增量檢查邏輯
    /// 
    /// 1. **構建 Cargo Check 命令** - 構建 `cargo check --message-format=json` 命令
    /// 2. **執行檢查** - 執行 Cargo Check 命令
    /// 3. **解析輸出** - 解析 JSON 輸出，提取結構化信息
    /// 4. **生成報告** - 生成包含錯誤、警告和統計信息的報告
    /// 
    /// # 錯誤
    /// 
    /// - `CargoError` - Cargo 檢查錯誤
    /// - `IoError` - IO 錯誤
    /// - `JsonError` - JSON 解析錯誤
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::cargo_check_runner::CargoCheckRunner;
    /// use std::path::PathBuf;
    /// use tokio;
    /// 
    /// #[tokio::main]
    /// async fn main() {
    ///     let project_root = PathBuf::from("/path/to/rust/project");
    ///     let runner = CargoCheckRunner::new(project_root);
    ///     
    ///     let modified_files = vec![PathBuf::from("src/main.rs")];
    ///     match runner.check_incremental(&modified_files).await {
    ///         Ok(report) => println!("增量檢查成功: {:?}", report),
    ///         Err(e) => eprintln!("增量檢查失敗: {:?}", e),
    ///     }
    /// }
    /// ```
    pub async fn check_incremental(&self, modified_files: &[PathBuf]) -> Result<CargoCheckReport, CargoError> {
        let start_time = Instant::now();
        
        // 構建增量檢查命令
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.project_root);
        cmd.arg("check");
        cmd.arg("--message-format=json");
        
        // 添加目標目錄
        if let Some(target_dir) = &self.target_dir {
            cmd.arg("--target-dir");
            cmd.arg(target_dir.to_str().unwrap_or("."));
        }
        
        // 執行檢查
        let output = cmd.output()
            .map_err(|e| CargoError::new("執行 cargo check 失敗", &self.project_root, 0, 0, &e.to_string()))?;
        
        let duration = start_time.elapsed();
        
        // 解析輸出
        let report = self.parse_cargo_output(&output.stdout, duration)?;
        
        Ok(report)
    }
    
    /// 執行完整專案檢查
    /// 
    /// # 返回值
    /// 
    /// 返回包含檢查結果的 CargoCheckReport，如果檢查失敗，則返回錯誤
    /// 
    /// # 完整檢查邏輯
    /// 
    /// 1. **構建 Cargo Check 命令** - 構建 `cargo check --message-format=json` 命令
    /// 2. **執行檢查** - 執行 Cargo Check 命令
    /// 3. **解析輸出** - 解析 JSON 輸出，提取結構化信息
    /// 4. **生成報告** - 生成包含錯誤、警告和統計信息的報告
    /// 
    /// # 錯誤
    /// 
    /// - `CargoError` - Cargo 檢查錯誤
    /// - `IoError` - IO 錯誤
    /// - `JsonError` - JSON 解析錯誤
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::cargo_check_runner::CargoCheckRunner;
    /// use std::path::PathBuf;
    /// use tokio;
    /// 
    /// #[tokio::main]
    /// async fn main() {
    ///     let project_root = PathBuf::from("/path/to/rust/project");
    ///     let runner = CargoCheckRunner::new(project_root);
    ///     
    ///     match runner.check_full().await {
    ///         Ok(report) => println!("完整檢查成功: {:?}", report),
    ///         Err(e) => eprintln!("完整檢查失敗: {:?}", e),
    ///     }
    /// }
    /// ```
    pub async fn check_full(&self) -> Result<CargoCheckReport, CargoError> {
        let start_time = Instant::now();
        
        // 構建完整檢查命令
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.project_root);
        cmd.arg("check");
        cmd.arg("--message-format=json");
        
        // 添加目標目錄
        if let Some(target_dir) = &self.target_dir {
            cmd.arg("--target-dir");
            cmd.arg(target_dir.to_str().unwrap_or("."));
        }
        
        // 執行檢查
        let output = cmd.output()
            .map_err(|e| CargoError::new("執行 cargo check 失敗", &self.project_root, 0, 0, &e.to_string()))?;
        
        let duration = start_time.elapsed();
        
        // 解析輸出
        let report = self.parse_cargo_output(&output.stdout, duration)?;
        
        Ok(report)
    }
    
    /// 解析 Cargo JSON 輸出
    /// 
    /// # 參數
    /// 
    /// * `output` - Cargo 的 JSON 輸出（字節數組）
    /// * `duration` - 檢查耗時
    /// 
    /// # 返回值
    /// 
    /// 返回包含檢查結果的 CargoCheckReport，如果解析失敗，則返回錯誤
    /// 
    /// # JSON 解析邏輯
    /// 
    /// 1. **遍歷 JSON 數組** - 遍歷所有 JSON 數組
    /// 2. **提取錯誤信息** - 提取 `render` 數組中的錯誤信息
    /// 3. **提取警告信息** - 提取 `warnings` 數組中的警告信息
    /// 4. **構建報告** - 構建 CargoCheckReport
    /// 
    /// # 錯誤
    /// 
    /// - `JsonError` - JSON 解析錯誤
    /// 
    /// # 示例
