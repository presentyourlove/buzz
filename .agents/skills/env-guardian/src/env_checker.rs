//! 環境變數檢查器模組
//! 
//! 負責驗證環境變數的完整性，包括必填變數、選填變數、格式驗證等
//! 
//! 主要功能包括：
//! 1. 環境變數規範定義和管理
//! 2. .env 檔案載入和系統環境變數檢查
//! 3. 格式驗證 (URL、Hex、路徑等)
//! 4. 敏感資訊保護和報告遮罩
//! 5. 修正建議腳本生成
//! 
//! # 使用範例
//! 
//! ```rust
//! use env_guardian::env_checker::{EnvChecker, EnvVarSpec, EnvValidator};
//! use std::path::PathBuf;
//! 
//! fn main() {
//!     // 定義環境變數規範
//!     let specs = vec![
//!         EnvVarSpec {
//!             name: "BUZZ_RELAY_URL".to_string(),
//!             required: true,
//!             description: "Nostr Relay 連線位址".to_string(),
//!             validator: Some(EnvValidator::Url { schemes: vec!["wss".to_string(), "ws".to_string()] }),
//!             default: None,
//!             secret: false,
//!         },
//!         EnvVarSpec {
//!             name: "BUZZ_PRIVATE_KEY".to_string(),
//!             required: true,
//!             description: "Agent 簽名私鑰 (hex)".to_string(),
//!             validator: Some(EnvValidator::Hex { length: Some(64) }),
//!             default: None,
//!             secret: true,
//!         },
//!         EnvVarSpec {
//!             name: "BUZZ_AUTH_TAG".to_string(),
//!             required: false,
//!             description: "Owner-reviewed agent drafts 授權碼".to_string(),
//!             validator: Some(EnvValidator::Hex { length: Some(32) }),
//!             default: None,
//!             secret: true,
//!         },
//!     ];
//!     
//!     let checker = EnvChecker::new(specs);
//!     let reports = checker.check_all();
//!     
//!     for report in reports {
//!         println!("變數: {:?}, 狀態: {:?}, 錯誤: {:?}", report.var_name, report.status, report.error);
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use crate::env_guardian::{config::EnvVarSpec, config::EnvValidator, config::CheckStatus, config::EnvCheckReport};

/// 環境變數檢查器
/// 
/// 負責驗證環境變數的完整性和格式，支援 .env 檔案載入、系統環境變數和預設值回退
/// 
/// # 功能
/// 
/// 1. **環境變數定義** - 管理環境變數的規範，包括必填、選填、驗證等
/// 2. **.env 檔案載入** - 載入 .env 檔案中的環境變數
/// 3. **系統環境變數檢查** - 檢查系統環境變數
/// 4. **格式驗證** - 驗證 URL、Hex、路徑等格式
/// 5. **敏感資訊保護** - 保護敏感資訊的報告顯示
/// 6. **修正建議** - 生成修正建議腳本
/// 
/// # 使用範例
/// 
/// ```rust
/// use env_guardian::env_checker::{EnvChecker, EnvVarSpec, EnvValidator};
/// 
/// fn main() {
///     // 定義環境變數規範
///     let specs = vec![
///         EnvVarSpec {
///             name: "BUZZ_RELAY_URL".to_string(),
///             required: true,
///             description: "Nostr Relay 連線位址".to_string(),
///             validator: Some(EnvValidator::Url { schemes: vec!["wss".to_string(), "ws".to_string()] }),
///             default: None,
///             secret: false,
///         },
///         EnvVarSpec {
///             name: "BUZZ_PRIVATE_KEY".to_string(),
///             required: true,
///             description: "Agent 簽名私鑰 (hex)".to_string(),
///             validator: Some(EnvValidator::Hex { length: Some(64) }),
///             default: None,
///             secret: true,
///         },
///     ];
///     
///     let checker = EnvChecker::new(specs);
///     let reports = checker.check_all();
///     
///     for report in reports {
///         println!("變數: {:?}, 狀態: {:?}, 錯誤: {:?}", report.var_name, report.status, report.error);
///     }
/// }
/// ```
pub struct EnvChecker {
    /// 環境變數規範
    specs: Vec<EnvVarSpec>,
}

impl EnvChecker {
    /// 創建新的環境變數檢查器
    /// 
    /// # 參數
    /// 
    /// * `specs` - 環境變數規範列表
    /// 
    /// # 返回值
    /// 
    /// 返回新的 EnvChecker 實例
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use env_guardian::env_checker::EnvChecker;
    /// use env_guardian::config::EnvVarSpec;
    /// use env_guardian::config::EnvValidator;
    /// 
    /// fn main() {
    ///     let specs = vec![
    ///         EnvVarSpec {
    ///             name: "BUZZ_RELAY_URL".to_string(),
    ///             required: true,
    ///             description: "Nostr Relay 連線位址".to_string(),
    ///             validator: Some(EnvValidator::Url { schemes: vec!["wss".to_string(), "ws".to_string()] }),
    ///             default: None,
    ///             secret: false,
    ///         },
    ///     ];
    ///     
    ///     let checker = EnvChecker::new(specs);
    ///     println!("創建了 EnvChecker");
    /// }
    /// ```
    pub fn new(specs: Vec<EnvVarSpec>) -> Self {
        Self { specs }
    }
    
    /// 檢查所有環境變數
    /// 
    /// # 返回值
    /// 
    /// 返回包含所有檢查結果的向量
    /// 
    /// # 檢查邏輯
    /// 
    /// 1. **載入 .env 檔案** - 載入 .env 檔案中的環境變數
    /// 2. **檢查系統環境變數** - 檢查系統環境變數
    /// 3. **檢查預設值** - 檢查預設值
    /// 4. **生成報告** - 生成檢查報告
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use env_guardian::env_checker::EnvChecker;
    /// use env_guardian::config::EnvVarSpec;
    /// use env_guardian::config::EnvValidator;
    /// 
    /// fn main() {
    ///     let specs = vec![
    ///         EnvVarSpec {
    ///             name: "BUZZ_RELAY_URL".to_string(),
    ///             required: true,
    ///             description: "Nostr Relay 連線位址".to_string(),
    ///             validator: Some(EnvValidator::Url { schemes: vec!["wss".to_string(), "ws".to_string()] }),
    ///             default: None,
    ///             secret: false,
    ///         },
    ///     ];
    ///     
    ///     let checker = EnvChecker::new(specs);
    ///     let reports = checker.check_all();
    ///     
    ///     for report in reports {
    ///         println!("變數: {:?}, 狀態: {:?}, 錯誤: {:?}", report.var_name, report.status, report.error);
    ///     }
    /// }
    /// ```
    pub fn check_all(&self) -> Vec<EnvCheckReport> {
        let mut reports = Vec::new();
        
        for spec in &self.specs {
            let report = self.check_env_var(spec);
            reports.push(report);
        }
        
        reports
    }
    
    /// 檢查單個環境變數
    /// 
    /// # 參數
    /// 
    /// * `spec` - 環境變數規範
    /// 
    /// # 返回值
    /// 
    /// 返回包含檢查結果的 EnvCheckReport
    /// 
    /// # 檢查邏輯
    /// 
    /// 1. **檢查系統環境變數** - 檢查系統環境變數
    /// 2. **檢查 .env 檔案** - 檢查 .env 檔案中的環境變數
    /// 3. **檢查預設值** - 檢查預設值
    /// 4. **驗證格式** - 驗證格式
    /// 5. **生成報告** - 生成檢查報告
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use env_guardian::env_checker::EnvChecker;
    /// use env_guardian::config::EnvVarSpec;
    /// use env_guardian::config::EnvValidator;
    /// 
    /// fn main() {
    ///     let spec = EnvVarSpec {
    ///         name: "BUZZ_RELAY_URL".to_string(),
    ///         required: true,
    ///         description: "Nostr Relay 連線位址".to_string(),
    ///         validator: Some(EnvValidator::Url { schemes: vec!["wss".to_string(), "ws".to_string()] }),
    ///         default: None,
    ///         secret: false,
    ///     };
    ///     
    ///     let checker = EnvChecker::new(vec![spec]);
    ///     let report = checker.check_env_var(&spec);
    ///     println!("變數: {:?}, 狀態: {:?}, 錯誤: {:?}", report.var_name, report.status, report.error);
    /// }
    /// ```
    fn check_env_var(&self, spec: &EnvVarSpec) -> EnvCheckReport {
        // 嘗試獲取環境變數
        let value = env::var(&spec.name).ok();
        
        // 如果沒有獲取到，檢查 .env 檔案
        let value = if value.is_none() {
            self.load_env_file(&spec.name)
        } else {
            value
        };
        
        // 如果還是沒有，檢查預設值
        let value = if value.is_none() {
            spec.default.clone()
        } else {
            value
        };
        
        // 執行驗證
        let (status, error, suggestion) = self.validate_env_var(spec, &value);
        
        // 生成預覽
        let value_preview = if spec.secret {
            None
        } else {
            value.as_ref().map(|v| {
                if v.len() > 8 {
                    format!("{}", &v[..8])
                } else {
                    v.clone()
                }
            })
        };
        
        EnvCheckReport {
            var_name: spec.name.clone(),
            status,
            value_preview,
            error,
            suggestion,
        }
    }
    
    /// 載入 .env 檔案
    fn load_env_file(&self, key: &str) -> Option<String> {
        let content = fs::read_to_string(".env").ok()?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
        None
    }
}
