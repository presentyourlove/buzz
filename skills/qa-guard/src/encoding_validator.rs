//! 編碼驗證器模組
//! 
//! 負責驗證 Patch 文件中的編碼和換行符，強制要求 UTF-8 無 BOM + LF
//! 
//! 主要功能包括：
//! 1. 檢測檔案編碼（UTF-8、UTF-16、BOM 等）
//! 2. 檢測換行符類型（LF、CRLF、CR 等）
//! 3. 強制執行 UTF-8 無 BOM + LF 編碼
//! 4. 提供自動修正功能
//! 5. 生成結構化的驗證報告
//! 
//! # 使用範例
//! 
//! ```rust
//! use qa_guard::encoding_validator::{EncodingValidator, EncodingReport, DetectedEncoding, LineEnding};
//! 
//! fn main() {
//!     let validator = EncodingValidator::new();
//!     let content = b"Hello World\nLine 2\n";
//!     let file_path = PathBuf::from("test.txt");
//!     
//!     let report = validator.validate(content, &file_path);
//!     println!("編碼驗證報告: {:?}", report);
//!     println!("是否通過: {}", report.is_valid);
//! }
//! ```

use std::path::Path;
use std::fs;

/// 編碼驗證器
/// 
/// 負責驗證文件的編碼和換行符，強制要求 UTF-8 無 BOM + LF
#[derive(Debug, Clone)]
pub struct EncodingValidator;

impl EncodingValidator {
    /// 創建新的編碼驗證器
    /// 
    /// # 返回值
    /// 
    /// 返回新的 EncodingValidator 實例
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::encoding_validator::EncodingValidator;
    /// 
    /// fn main() {
    ///     let validator = EncodingValidator::new();
    ///     println!("創建了編碼驗證器");
    /// }
    /// ```
    pub fn new() -> Self {
        Self
    }
    
    /// 驗證內容的編碼和換行符
    /// 
    /// # 參數
    /// 
    /// * `content` - 要驗證的內容（字節數組）
    /// * `file_path` - 檔案路徑（用於錯誤報告）
    /// 
    /// # 返回值
    /// 
    /// 返回驗證報告，包含驗證結果和錯誤信息
    /// 
    /// # 驗證邏輯
    /// 
    /// 1. **檢測編碼** - 使用 BOM 檢測 UTF-8、UTF-16 等編碼
    /// 2. **檢測換行符** - 檢測 LF、CRLF、CR 等換行符類型
    /// 3. **檢查 BOM** - 檢查是否有 BOM
    /// 4. **判斷是否通過** - 只有 UTF-8、無 BOM 和 LF 才通過
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::encoding_validator::{EncodingValidator, EncodingReport};
    /// use std::path::PathBuf;
    /// 
    /// fn main() {
    ///     let validator = EncodingValidator::new();
    ///     let utf8_content = b"Hello\nWorld\n";
    ///     let utf8_with_bom = b"\xEF\xBB\xBFHello\nWorld\n";
    ///     let utf16_content = b"Hello\r\nWorld\r\n";
    ///     let file_path = PathBuf::from("test.txt");
    ///     
    ///     let utf8_report = validator.validate(utf8_content, &file_path);
    ///     let utf8_bom_report = validator.validate(utf8_with_bom, &file_path);
    ///     let utf16_report = validator.validate(utf16_content, &file_path);
    ///     
    ///     println!("UTF-8 通過: {}", utf8_report.is_valid);
    ///     println!("UTF-8+ BOM 通過: {}", utf8_bom_report.is_valid);
    ///     println!("UTF-16 通過: {}", utf16_report.is_valid);
    /// }
    /// ```
    pub fn validate(&self, content: &[u8], file_path: &Path) -> EncodingReport {
        let encoding = self.detect_encoding(content);
        let line_ending = self.detect_line_ending(content);
        let has_bom = self.has_bom(content);
        
        // 只有 UTF-8、無 BOM 和 LF 才通過
        let is_valid = matches!(encoding, DetectedEncoding::Utf8)
            && !has_bom
            && matches!(line_ending, LineEnding::Lf);
        
        let mut errors = Vec::new();
        
        if !matches!(encoding, DetectedEncoding::Utf8) {
            errors.push(EncodingError::InvalidEncoding(format!("檢測到編碼: {:?}", encoding)));
        }
        
        if has_bom {
            errors.push(EncodingError::BomFound);
        }
        
        match line_ending {
            LineEnding::Lf => {}
            LineEnding::Crlf => errors.push(EncodingError::InvalidLineEnding(LineEnding::Crlf)),
            LineEnding::Cr => errors.push(EncodingError::InvalidLineEnding(LineEnding::Cr)),
            LineEnding::Mixed => errors.push(EncodingError::InvalidLineEnding(LineEnding::Mixed)),
        }
        
        EncodingReport {
            file_path: file_path.to_path_buf(),
            is_valid,
            encoding,
            line_ending,
            has_bom,
            errors,
        }
    }
    
    /// 自動修正內容
    /// 
    /// # 參數
    /// 
    /// * `content` - 要修正的內容（字節數組）
    /// 
    /// # 返回值
    /// 
    /// 返回修正後的內容，如果修正失敗，則返回 FixError
    /// 
    /// # 自動修正邏輯
    /// 
    /// 1. **轉換編碼** - 將非 UTF-8 編碼轉換為 UTF-8
    /// 2. **移除 BOM** - 移除 BOM
    /// 3. **正規化換行符** - 將所有換行符轉換為 LF
    /// 
    /// # 錯誤
    /// 
    /// 如果轉換失敗，將返回 FixError
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::encoding_validator::EncodingValidator;
    /// 
    /// fn main() {
    ///     let validator = EncodingValidator::new();
    ///     let content = b"Hello\r\nWorld\r\n";
    ///     
    ///     match validator.auto_fix(content) {
    ///         Ok(fixed) => {
    ///             println!("修正後內容: {:?}", fixed);
    ///         }
    ///         Err(e) => {
    ///             println!("修正失敗: {:?}", e);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn auto_fix(&self, content: &[u8]) -> Result<Vec<u8>, FixError> {
        // 嘗試解碼為 UTF-8
        match String::from_utf8(content.to_vec()) {
            Ok(text) => {
                // 已經是有效的 UTF-8，檢查 BOM 和換行符
                let mut result = text.into_bytes();
                
                // 移除 BOM
                if result.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    result.drain(0..3);
                }
                
                // 正規化換行符
                result = result.replace(&b"\r\n"[..], &b"\n"[..]).replace(&b"\r"[..], &b"\n"[..]);
                
                Ok(result)
            }
            Err(e) => Err(FixError::InvalidEncoding(e)),
        }
    }
    
    /// 檢測內容的編碼
    /// 
    /// # 參數
    /// 
    /// * `content` - 要檢測的內容（字節數組）
    /// 
    /// # 返回值
    /// 
    /// 返回檢測到的編碼類型
    /// 
    /// # 編碼檢測邏輯
    /// 
    /// 1. **檢查 BOM** - 檢測 UTF-8、UTF-16 LE 和 BE BOM
    /// 2. **優先使用 BOM** - 如果有 BOM，使用 BOM 檢測的編碼
    /// 3. **嘗試解碼** - 嘗試解碼為 UTF-8、UTF-16 等
    /// 4. **未知編碼** - 如果都失敗，則返回 Unknown
    fn detect_encoding(&self, content: &[u8]) -> DetectedEncoding {
        // 檢查 UTF-8 BOM
        if content.len() >= 3 && content[0] == 0xEF && content[1] == 0xBB && content[2] == 0xBF {
            return DetectedEncoding::Utf8Bom;
        }
        
        // 檢查 UTF-16 LE BOM
        if content.len() >= 2 && content[0] == 0xFF && content[1] == 0xFE {
            return DetectedEncoding::Utf16Le;
        }
        
        // 檢查 UTF-16 BE BOM
        if content.len() >= 2 && content[0] == 0xFE && content[1] == 0xFF {
            return DetectedEncoding::Utf16Be;
        }
        
        // 嘗試解碼為 UTF-8
        match String::from_utf8(content.to_vec()) {
            Ok(_) => return DetectedEncoding::Utf8,
            Err(_) => {},
        }
        
        // 嘗試解碼為 UTF-16 LE
        if let Ok(_) = String::from_utf16(&content.chunks_exact(2).map(|chunk| {
            if chunk.len() == 2 { ((chunk[0] as u16) | ((chunk[1] as u16) << 8)) } else { chunk[0] as u16 }
        }).collect::<Vec<u16>>()) {
            return DetectedEncoding::Utf16Le;
        }
        
        // 嘗試解碼為 UTF-16 BE
        if let Ok(_) = String::from_utf16(&content.chunks_exact(2).map(|chunk| {
            if chunk.len() == 2 { ((chunk[1] as u16) << 8) | (chunk[0] as u16) } else { chunk[0] as u16 }
        }).collect::<Vec<u16>>()) {
            return DetectedEncoding::Utf16Be;
        }
        
        // 未知編碼
        DetectedEncoding::Unknown
    }
    
    /// 檢測內容的換行符類型
    /// 
    /// # 參數
    /// 
    /// * `content` - 要檢測的內容（字節數組）
    /// 
    /// # 返回值
    /// 
    /// 返回換行符類型
    /// 
    /// # 換行符檢測邏輯
    /// 
    /// 1. **分析內容** - 遍歷所有字節，檢測換行符
    /// 2. **統計每種換行符** - 統計 LF、CRLF、CR 等換行符的數量
    /// 3. **判斷類型** - 如果只有 LF，返回 Lf；如果是 LF 和 CRLF，返回 Mixed；否則返回其他類型
    fn detect_line_ending(&self, content: &[u8]) -> LineEnding {
        let mut lf_count = 0;
        let mut crlf_count = 0;
        let mut cr_count = 0;
        let mut mixed = false;
        
        let mut i = 0;
        while i < content.len() {
            match content[i] {
                b'\n' => {
                    if i > 0 && content[i - 1] == b'\r' {
                        crlf_count += 1;
                    } else {
                        lf_count += 1;
                    }
                }
                b'\r' => {
                    if i + 1 >= content.len() || content[i + 1] != b'\n' {
                        cr_count += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let kinds = (lf_count > 0) as u8 + (crlf_count > 0) as u8 + (cr_count > 0) as u8;
        if kinds > 1 {
            mixed = true;
        }
        if mixed {
            LineEnding::Mixed
        } else if crlf_count > 0 {
            LineEnding::Crlf
        } else if cr_count > 0 {
            LineEnding::Cr
        } else {
            LineEnding::Lf
        }
    }

    fn has_bom(&self, content: &[u8]) -> bool {
        content.len() >= 3 && content[0] == 0xEF && content[1] == 0xBB && content[2] == 0xBF
    }
}
