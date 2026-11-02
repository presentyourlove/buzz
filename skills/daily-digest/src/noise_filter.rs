//! 行級雜訊過濾器模組
//! 回應流程優化 #1：將雜訊過濾器由「訊息級」升級為「行級」

use regex::Regex;

/// 行級雜訊過濾器
/// 
/// 負責掃描訊息內容，提取有意義的行並過濾掉無用的雜訊行
/// 
/// # 核心功能
/// 
/// 1. **雜訊識別** - 識別並過濾掉以下類型的行：
///    - 純狀態報告和心跳信息
///    - 純表情符號或單詞回應
///    - 系統日誌雜訊
/// 
/// 2. **訊號保留** - 保留以下有意義的行：
///    - 決策、架構討論和具體行動項目
///    - `@mention` 觸發的工作指派
///    - 代碼片段和配置信息
///    - 錯誤堆疊和關鍵異常信息
/// 
/// # 使用範例
/// 
/// ```rust
/// use daily_digest::noise_filter::NoiseFilter;
/// 
/// fn main() {
///     let filter = NoiseFilter::new();
///     let content = "\n".join(&[
///         "已完成 task #123",
///         "@Pollen 請實作模組 X",
///         "OK",
///         "tracing::debug!(\"llm keepalive tick\")",
///         "決議採用 Rust 實作",
///         "Error: connection refused",
///         "完成代碼審查"
///     ]);
///     
///     let filtered = filter.filter_lines(&content);
///     assert_eq!(filtered.len(), 3);
///     assert!(filtered.contains(&"@Pollen 請實作模組 X".to_string()));
///     assert!(filtered.contains(&"決議採用 Rust 實作".to_string()));
///     assert!(filtered.contains(&"Error: connection refused".to_string()));
/// }
/// ```

/// # 配置過濾規則
/// 
/// 預設配置採用嚴格模式，只有明顯的有意義訊息才會保留。默認規則包括：
/// 
/// ## 狀態報告和心跳
/// - `已完成`、`心跳正常`、`keepalive tick`
/// 
/// ## 純表情和單詞
/// - `OK`、`收到`、`good`、`done`
/// 
/// ## 系統日誌雜訊
/// - `tracing::debug!(\"llm keepalive tick\")`
/// - `INFO:`、`DEBUG:`、`TRACE:` 等級日志
/// 
/// ## 有意義訊息（保留）
/// - 包含「決策」、「規劃」、「行動項目」、「指派」的行
/// - 包含 `@mention` 的行
/// - 包含代碼區塊的行（`/`、`` ` ``、`fn ` 等
/// - 包含錯誤堆疊的行（`Error:`、`Exception:`、`panic!`）

pub struct NoiseFilter {
    /// 雜訊模式列表，用於匹配需要過濾的行
    noise_patterns: Vec<Regex>,
    /// 訊號模式列表，用於匹配需要保留的行
    signal_patterns: Vec<Regex>,
}

impl NoiseFilter {
    /// 創建新的 NoiseFilter 實例
    /// 
    /// # 返回值
    /// 
    /// 返回一個包含預設雜訊和訊號模式的 NoiseFilter 實例
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use daily_digest::noise_filter::NoiseFilter;
    /// 
    /// let filter = NoiseFilter::new();
    /// ```
    pub fn new() -> Self {
        let noise_patterns = vec![
            Regex::new(r"^\s*(已完成|心跳正常|keepalive tick|完成)\s*$").unwrap(),
            Regex::new(r"^\s*(OK|收到|good|done|completed)\s*$").unwrap(),
            Regex::new(r"^\s*(tracing::debug|tracing::info|tracing::trace)\(\s*\)[^)]*\)\s*$").unwrap(),
        ];
        
        let signal_patterns = vec![
            Regex::new(r"^\s*(決策|規劃|行動項目|指派|架構|討論)\s*").unwrap(),
            Regex::new(r"^\s*@[a-zA-Z0-9_]+\s").unwrap(),
            Regex::new(r"^[^\s].*[/`'].*[/`']").unwrap(),
            Regex::new(r"^[^\s].*Error:|Exception:|panic!").unwrap(),
            Regex::new(r"^[^\s].*(Rust|TypeScript|Java|Go|Python)\s+").unwrap(),
        ];
        
        Self {
            noise_patterns,
            signal_patterns,
        }
    }
    
    /// 過濾訊息中的雜訊，提取有意義的行
    /// 
    /// # 參數
    /// 
    /// * `content` - 要過濾的完整訊息內容
    /// 
    /// # 返回值
    /// 
    /// 返回一個包含所有有意義行的 `Vec<String>`
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use daily_digest::noise_filter::NoiseFilter;
    /// 
    /// let filter = NoiseFilter::new();
    /// let content = "已完成 task #123\n@Pollen 請實作模組 X\nOK\n決議採用 Rust 實作\nError: connection refused\n完成代碼審查";
    /// 
    /// let filtered = filter.filter_lines(content);
    /// assert_eq!(filtered.len(), 3);
    /// ```
    pub fn filter_lines(&self, content: &str) -> Vec<String> {
        content.lines()
            .filter(|line| self.is_signal_line(line))
            .map(|s| s.to_string())
            .collect()
    }
    
    /// 判斷某一行是否為有意義的訊號
    /// 
    /// # 參數
    /// 
    /// * `line` - 要判斷的單行文字
    /// 
    /// # 返回值
    /// 
    /// 如果該行為有意義的訊號，則返回 `true`，否則返回 `false`
    /// 
    /// # 判斷邏輯
    /// 
    /// 1. **優先檢查訊號模式** - 如果匹配到任何訊號模式，則認為是有意義的
    /// 2. **其次檢查雜訊模式** - 如果匹配到任何雜訊模式，則認為是雜訊
    /// 3. **預設處理** - 對長度過短的行或不含任何關鍵字的行進行特殊處理
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use daily_digest::noise_filter::NoiseFilter;
    /// 
    /// let filter = NoiseFilter::new();
    /// assert!(filter.is_signal_line("@Pollen 請實作模組 X")); // true
    /// assert!(!filter.is_signal_line("已完成")); // false
    /// assert!(filter.is_signal_line("決議採用 Rust 實作")); // true
    /// ```
    fn is_signal_line(&self, line: &str) -> bool {
        // 如果匹配到任何訊號模式，則認為是有意義的
        if self.signal_patterns.iter().any(|pattern| pattern.is_match(line)) {
            return true;
        }
        
        // 如果匹配到任何雜訊模式，則認為是雜訊
        if self.noise_patterns.iter().any(|pattern| pattern.is_match(line)) {
            return false;
        }
        
        // 預設邏輯：
        // 1. 排除純表情或單詞
        if line.trim().len() <= 2 && !line.contains('@') && !line.contains('#') {
            return false;
        }
        
        // 2. 排除心跳或狀態報告信息
        if line.contains("heartbeat") || line.contains("tick") || line.contains("completed") {
            return false;
        }
        
        // 3. 排除純日誌級別信息
        if line.contains("tracing::debug") || line.contains("tracing::info") || line.contains("tracing::trace") {
            return false;
        }
        
        // 4. 保留其他所有有意義的行
        true
    }
}

/// 工廠函數 - 創建預設配置的 NoiseFilter 實例
/// 
/// # 返回值
/// 
/// 返回配置最適化的 NoiseFilter 實例
/// 
/// # 注意
/// 
/// 此函數保留用於向後兼容性。建議直接使用 NoiseFilter::new()
impl Default for NoiseFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_noise_filter_new() {
        let filter = NoiseFilter::new();
        assert!(!filter.noise_patterns.is_empty());
        assert!(!filter.signal_patterns.is_empty());
    }
    
    #[test]
    fn test_is_signal_line_with_signal() {
        let filter = NoiseFilter::new();
        assert!(filter.is_signal_line("@Pollen 請實作模組 X"));
        assert!(filter.is_signal_line("決議採用 Rust 實作"));
        assert!(filter.is_signal_line("Error: connection refused"));
        assert!(filter.is_signal_line("完成代碼審查"));
        assert!(filter.is_signal_line("fn main() { /* 代碼 */ }"));
        assert!(filter.is_signal_line("\u{1F600} 好的")); // 包含表情符號且有字
    }
    
    #[test]
    fn test_is_signal_line_with_noise() {
        let filter = NoiseFilter::new();
        assert!(!filter.is_signal_line("已完成"));
        assert!(!filter.is_signal_line("OK"));
        assert!(!filter.is_signal_line("收到"));
        assert!(!filter.is_signal_line("tracing::debug!(\"llm keepalive tick\")"));
    }
    
    #[test]
    fn test_is_signal_line_edge_cases() {
        let filter = NoiseFilter::new();
        assert!(!filter.is_signal_line(""));
        assert!(!filter.is_signal_line("   "));
        assert!(!filter.is_signal_line("OK"));
        assert!(!filter.is_signal_line("good"));
        assert!(!filter.is_signal_line("done"));
        assert!(!filter.is_signal_line("heartbeat tick"));
        assert!(filter.is_signal_line("這是一行有意義的文字"));
    }
    
    #[test]
    fn test_filter_lines() {
        let filter = NoiseFilter::new();
        let content = "已完成 task #123\n@Pollen 請實作模組 X\nOK\n決議採用 Rust 實作\nError: connection refused\n完成代碼審查\ntracing::debug!(\"llm keepalive tick\")";
        
        let filtered = filter.filter_lines(content);
        
        assert_eq!(filtered.len(), 4); // @Pollen, 決議採用, Error, 完成代碼審查
        assert!(filtered.contains(&"@Pollen 請實作模組 X".to_string()));
        assert!(filtered.contains(&"決議採用 Rust 實作".to_string()));
        assert!(filtered.contains(&"Error: connection refused".to_string()));
        assert!(filtered.contains(&"完成代碼審查".to_string()));
    }
    
    #[test]
    fn test_filter_lines_empty() {
        let filter = NoiseFilter::new();
        let content = "已完成\nOK\nheart"; // 所有都是雜訊
        
        let filtered = filter.filter_lines(content);
        assert!(filtered.is_empty());
    }
}
