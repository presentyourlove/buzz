use std::path::Path;

/// Encoding Validator
/// 核心功能：驗證文件編碼，確保編碼合規性，並提供編碼轉換建議
pub struct EncodingValidator {
    /// 支持的編碼格式列表
    supported_encodings: Vec<String>,
    /// 編碼檢查配置
    config: ValidationConfig,
}

/// 驗證配置結構
pub struct ValidationConfig {
    /// 是否嚴格檢查編碼
    pub strict_mode: bool,
    /// 是否驗證 BOM（字節順序標記）
    pub check_bom: bool,
    /// 是否檢查編碼兼容性
    pub check_compatibility: bool,
    /// 是否自動修復編碼
    pub auto_fix: bool,
}

/// 文件編碼信息結構
pub struct FileEncodingInfo {
    /// 文件路徑
    pub file_path: String,
    /// 文件編碼
    pub encoding: String,
    /// 是否檢測到 BOM
    pub has_bom: bool,
    /// 文件大小
    pub file_size: u64,
    /// 是否需要轉換
    pub needs_conversion: bool,
    /// 建議的編碼
    pub suggested_encoding: String,
    /// 驗證結果
    pub validation_result: ValidationResult,
}

/// 驗證結果結構
pub struct ValidationResult {
    /// 是否通過驗證
    pub is_valid: bool,
    /// 錯誤信息
    pub errors: Vec<String>,
    /// 警告信息
    pub warnings: Vec<String>,
    /// 建議的修復操作
    pub suggested_fixes: Vec<String>,
}

impl EncodingValidator {
    /// 創建新的編碼驗證器實例
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            supported_encodings: vec![
                "UTF-8".to_string(),
                "UTF-8 BOM".to_string(),
                "UTF-16LE".to_string(),
                "UTF-16BE".to_string(),
                "ASCII".to_string(),
                "ISO-8859-1".to_string(),
            ],
            config,
        }
    }

    /// 驗證文件編碼
    pub fn validate_file_encoding(&self, file_path: &str) -> Result<FileEncodingInfo, ValidationError> {
        let path = Path::new(file_path);
        
        if !path.exists() {
            return Err(ValidationError::FileNotFound(file_path.to_string()));
        }
        
        if !path.is_file() {
            return Err(ValidationError::NotAFile(file_path.to_string()));
        }
        
        // 讀取文件並檢測編碼
        let file_size = path.metadata()
            .map_err(|e| ValidationError::FileMetadataError(file_path.to_string(), e.to_string()))?
            .len();
        
        let content = std::fs::read(path)
            .map_err(|e| ValidationError::FileReadError(file_path.to_string(), e.to_string()))?;
        
        let (encoding, has_bom) = self.detect_encoding(&content);
        let suggested_encoding = self.suggest_encoding(&encoding, &content);
        let needs_conversion = suggested_encoding != encoding;
        
        // 驗證編碼
        let validation_result = self.validate_encoding_quality(&encoding, &content);
        
        Ok(FileEncodingInfo {
            file_path: file_path.to_string(),
            encoding,
            has_bom,
            file_size,
            needs_conversion,
            suggested_encoding,
            validation_result,
        })
    }

    /// 檢測文件編碼
    fn detect_encoding(&self, content: &[u8]) -> (String, bool) {
        let content_str = String::from_utf8(content.to_vec());
        
        if content_str.is_ok() {
            // 檢查是否有 UTF-8 BOM
            if content.len() >= 3 && &content[0..3] == [0xEF, 0xBB, 0xBF] {
                ("UTF-8 BOM".to_string(), true)
            } else {
                ("UTF-8".to_string(), false)
            }
        } else {
            // 嘗試檢測其他編碼
            self.detect_other_encodings(content)
        }
    }

    /// 檢測其他編碼
    fn detect_other_encodings(&self, content: &[u8]) -> (String, bool) {
        // 簡單的編碼檢測邏輯
        let mut utf8_valid = true;
        let mut utf8_bom = false;
        let mut utf16le_valid = true;
        let mut utf16be_valid = true;
        
        if content.len() >= 3 && &content[0..3] == [0xEF, 0xBB, 0xBF] {
            utf8_bom = true;
        }
        
        // 嘗試解析為 UTF-8
        if let Ok(_) = String::from_utf8(content.to_vec()) {
            utf8_valid = true;
        }
        
        // 嘗試解析為 UTF-16LE
        if let Ok(_) = String::from_utf16(&Self::bytes_to_u16le(content)) {
            utf16le_valid = true;
        }
        
        // 嘗試解析為 UTF-16BE
        if let Ok(_) = String::from_utf16(&Self::bytes_to_u16be(content)) {
            utf16be_valid = true;
        }
        
        if utf8_valid && utf8_bom {
            ("UTF-8 BOM".to_string(), true)
        } else if utf8_valid {
            ("UTF-8".to_string(), false)
        } else if utf16le_valid {
            ("UTF-16LE".to_string(), false)
        } else if utf16be_valid {
            ("UTF-16BE".to_string(), false)
        } else {
            ("Unknown".to_string(), false)
        }
    }

    /// 將字节轉換為 UTF-16LE 數組
    fn bytes_to_u16le(&self, bytes: &[u8]) -> Vec<u16> {
        let mut result = Vec::new();
        let mut i = 0;
        
        while i + 1 < bytes.len() {
            let byte1 = bytes[i] as u16;
            let byte2 = bytes[i + 1] as u16;
            result.push(byte1 | (byte2 << 8));
            i += 2;
        }
        
        result
    }

    /// 將字节轉換為 UTF-16BE 數組
    fn bytes_to_u16be(&self, bytes: &[u8]) -> Vec<u16> {
        let mut result = Vec::new();
        let mut i = 0;
        
        while i + 1 < bytes.len() {
            let byte1 = bytes[i] as u16;
            let byte2 = bytes[i + 1] as u16;
            result.push((byte1 << 8) | byte2);
            i += 2;
        }
        
        result
    }

    /// 建議合適的編碼
    fn suggest_encoding(&self, current_encoding: &str, content: &[u8]) -> String {
        if self.supported_encodings.contains(&current_encoding.to_string()) {
            current_encoding.to_string()
        } else {
            // 如果當前編碼不受支持，建議 UTF-8
            "UTF-8".to_string()
        }
    }

    /// 驗證編碼質量
    fn validate_encoding_quality(&self, encoding: &str, content: &[u8]) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggested_fixes = Vec::new();
        
        if encoding == "Unknown" {
            errors.push("Cannot detect file encoding".to_string());
            suggested_fixes.push("Convert to UTF-8 encoding".to_string());
        } else if encoding != "UTF-8" && encoding != "UTF-8 BOM" {
            warnings.push(format!("File uses {} encoding, but UTF-8 is recommended", encoding));
            suggested_fixes.push("Convert to UTF-8 for better compatibility".to_string());
        }
        
        if self.config.check_bom {
            if encoding == "UTF-8 BOM" && !self.config.strict_mode {
                warnings.push("File contains UTF-8 BOM, which may cause compatibility issues".to_string());
                suggested_fixes.push("Remove UTF-8 BOM if not needed".to_string());
            }
        }
        
        if self.config.check_compatibility {
            if !self.is_encoding_compatible(encoding) {
                errors.push(format!("Encoding {} is not compatible with the system", encoding));
                suggested_fixes.push("Convert to a supported encoding".to_string());
            }
        }
        
        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            suggested_fixes,
        }
    }

    /// 檢查編碼是否兼容
    fn is_encoding_compatible(&self, encoding: &str) -> bool {
        self.supported_encodings.contains(&encoding.to_string())
    }

    /// 批量驗證多個文件
    pub fn validate_multiple_files(&self, file_paths: &[&str]) -> Vec<Result<FileEncodingInfo, ValidationError>> {
        file_paths.iter().map(|path| self.validate_file_encoding(path)).collect()
    }

    /// 獲取驗證統計信息
    pub fn get_validation_stats(&self, results: &[Result<FileEncodingInfo, ValidationError>]) -> ValidationStats {
        let mut total_files = 0;
        let mut valid_files = 0;
        let mut needs_conversion = 0;
        let mut encoding_stats: HashMap<String, usize> = HashMap::new();
        
        for result in results {
            total_files += 1;
            match result {
                Ok(info) => {
                    valid_files += 1;
                    if info.needs_conversion {
                        needs_conversion += 1;
                    }
                    *enc
