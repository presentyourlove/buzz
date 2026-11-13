//! 行級雜訊過濾器模組
//! 回應流程優化 #1：將雜訊過濾器由「訊息級」升級為「行級」
//! Noise Filter v2：新增 Q&A/閒聊識別、啟發式權重評分、可配置信號閾值

use regex::Regex;

/// 雜訊分類枚舉
///
/// 用於標記每一行的分類結果，支援 Noise Filter v2 的精細分類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseCategory {
    /// 決策類 - 包含決議、選擇方案等
    Decision,
    /// 行動項目類 - 包含具體任務、指派、待辦等
    ActionItem,
    /// 提及類 - 包含 @mention 工作指派
    Mention,
    /// 代碼類 - 包含代碼片段、配置、函式定義等
    Code,
    /// 錯誤類 - 包含錯誤堆疊、異常、失敗信息等
    Error,
    /// 問答/閒聊類 - 包含問題、閒聊、問候、元評論等（Noise Filter v2 新增）
    QaChat,
    /// 純雜訊 - 心跳、狀態報告、單詞回應等
    Noise,
    /// 未分類 - 無法明確歸類的行
    Unclassified,
}

/// 行級雜訊過濾器
///
/// 負責掃描訊息內容，提取有意義的行並過濾掉無用的雜訊行
#[derive(Clone)]
pub struct NoiseFilter {
    /// 雜訊模式列表，用於匹配需要過濾的行
    pub noise_patterns: Vec<Regex>,
    /// 訊號模式列表，用於匹配需要保留的行
    pub signal_patterns: Vec<Regex>,
    /// 問答/閒聊特定模式（Noise Filter v2）
    qa_patterns: Vec<Regex>,
    /// 簡短確認與心跳日誌模式
    ack_patterns: Vec<Regex>,
    /// 決策/規劃/行動項目模式
    decision_action_patterns: Vec<Regex>,
    /// @mention 模式
    mention_patterns: Vec<Regex>,
    /// 代碼/技術/錯誤模式
    code_tech_patterns: Vec<Regex>,
    /// 訊號閾值（預設為 0；評分 >= 閾值則保留）
    pub signal_threshold: i32,
}

impl NoiseFilter {
    /// 創建新的 NoiseFilter 實例（預設閾值 0）
    pub fn new() -> Self {
        Self::with_threshold(0)
    }

    /// 創建指定訊號閾值的 NoiseFilter 實例
    pub fn with_threshold(signal_threshold: i32) -> Self {
        let ack_patterns = vec![
            // 狀態報告和心跳（包含已完成開頭的狀態行）
            Regex::new(r"^\s*(已完成|心跳正常|keepalive tick|heartbeat tick)").unwrap(),
            Regex::new(r"^\s*完成\s*$").unwrap(),
            // 簡短確認/回應
            Regex::new(r"^\s*(OK|收到|good|done|completed|好的|沒問題|沒關係|隨意|無所謂)\s*$").unwrap(),
            // 系統日誌雜訊
            Regex::new(r"^\s*(tracing::debug|tracing::info|tracing::trace)\(\s*.*?\)\s*$").unwrap(),
            Regex::new(r"tracing::debug!").unwrap(),
        ];

        let qa_patterns = vec![
            // Q&A 標籤開頭
            Regex::new(r"^\s*(Q:|A:|問[：:]|答[：:])").unwrap(),
            // 疑問詞開頭
            Regex::new(r"^\s*(為什麼|如何|請問|想問|可否|能否|能不能|有沒有|是不是)").unwrap(),
            Regex::new(r"^\s*(\?|？)").unwrap(),
            // 客套/感謝/問候
            Regex::new(r"^\s*(謝謝|感謝|早安|午安|晚安|哈囉|你好|嗨)").unwrap(),
            // 問句結尾（純問句且不包含決策或程式碼標記）
            Regex::new(r".+[？?]\s*$").unwrap(),
        ];

        let mut noise_patterns = Vec::new();
        noise_patterns.extend(ack_patterns.clone());
        noise_patterns.extend(qa_patterns.clone());

        let mention_patterns = vec![
            Regex::new(r"@[a-zA-Z0-9_\-\u4e00-\u9fa5]+").unwrap(),
        ];

        let decision_action_patterns = vec![
            Regex::new(r"(決策|決議|規劃|行動項目|指派|架構|討論|方案|選擇|採用|決定|確認|完成.*審查|任務.*[:：]|TODO[:：]|FIXME[:：])").unwrap(),
        ];

        let code_tech_patterns = vec![
            Regex::new(r"[^\s].*[/`'].*[/`']").unwrap(),
            Regex::new(r"```").unwrap(),
            Regex::new(r"^\s*(fn|async\s+fn|let|const|var|function|def|import|use)\s+\w+").unwrap(),
            Regex::new(r"Error:|Exception:|panic!|failed").unwrap(),
            Regex::new(r"(Rust|TypeScript|Java|Go|Python|Docker|Kubernetes)").unwrap(),
        ];

        let mut signal_patterns = Vec::new();
        signal_patterns.extend(decision_action_patterns.clone());
        signal_patterns.extend(mention_patterns.clone());
        signal_patterns.extend(code_tech_patterns.clone());

        Self {
            noise_patterns,
            signal_patterns,
            qa_patterns,
            ack_patterns,
            decision_action_patterns,
            mention_patterns,
            code_tech_patterns,
            signal_threshold,
        }
    }

    /// 啟發式評分單行內容
    ///
    /// # 權重機制
    /// - +2: 包含 `@mention`
    /// - +2: 包含決策/規劃/行動/架構等關鍵字
    /// - +1: 包含代碼片段、技術關鍵字或錯誤堆疊
    /// - -2: 匹配 Q&A/問句/閒聊模式
    /// - -1: 匹配純確認/心跳/日誌
    /// - 預設為 0
    pub fn score_line(&self, line: &str) -> i32 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return -5;
        }

        // 長度過短且無特殊標記視為雜訊
        if trimmed.chars().count() <= 2 && !trimmed.contains('@') && !trimmed.contains('#') {
            return -2;
        }

        let mut score = 0;

        // 訊號特徵加分
        if self.mention_patterns.iter().any(|p| p.is_match(trimmed)) {
            score += 2;
        }

        if self.decision_action_patterns.iter().any(|p| p.is_match(trimmed)) {
            score += 2;
        }

        if self.code_tech_patterns.iter().any(|p| p.is_match(trimmed)) {
            score += 1;
        }

        // 雜訊特徵扣分
        if self.qa_patterns.iter().any(|p| p.is_match(trimmed)) {
            score -= 2;
        }

        if self.ack_patterns.iter().any(|p| p.is_match(trimmed))
            || trimmed.contains("heartbeat")
            || trimmed.contains("tick")
            || trimmed.contains("tracing::")
        {
            score -= 1;
        }

        score
    }

    /// 判斷某一行是否為有意義的訊號
    pub fn is_signal_line(&self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }

        // 優先過濾掉明確純雜訊（如單詞 OK、收到、心跳）
        if self.ack_patterns.iter().any(|p| p.is_match(trimmed)) && !self.decision_action_patterns.iter().any(|p| p.is_match(trimmed)) {
            return false;
        }

        // 依據啟發式評分判斷是否達到保留閾值
        self.score_line(line) >= self.signal_threshold
    }

    /// 過濾訊息中的雜訊，提取有意義的行
    pub fn filter_lines(&self, content: &str) -> Vec<String> {
        content
            .lines()
            .filter(|line| self.is_signal_line(line))
            .map(|s| s.to_string())
            .collect()
    }

    /// 過濾訊息並返回附帶啟發式評分的結果元組 (Line, Score)
    pub fn filter_lines_with_scores(&self, content: &str) -> Vec<(String, i32)> {
        content
            .lines()
            .filter(|line| self.is_signal_line(line))
            .map(|s| (s.to_string(), self.score_line(s)))
            .collect()
    }
}

impl Default for NoiseFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_filter_creation() {
        let filter = NoiseFilter::new();
        assert!(!filter.noise_patterns.is_empty());
        assert!(!filter.signal_patterns.is_empty());
        assert_eq!(filter.signal_threshold, 0);
    }

    #[test]
    fn test_is_signal_line_with_signal() {
        let filter = NoiseFilter::new();
        assert!(filter.is_signal_line("@Pollen 請實作模組 X"));
        assert!(filter.is_signal_line("決議採用 Rust 實作"));
        assert!(filter.is_signal_line("Error: connection refused"));
        assert!(filter.is_signal_line("完成代碼審查"));
        assert!(filter.is_signal_line("fn main() { /* 代碼 */ }"));
        assert!(filter.is_signal_line("\u{1F600} 好的"));
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

        assert_eq!(filtered.len(), 4);
        assert!(filtered.contains(&"@Pollen 請實作模組 X".to_string()));
        assert!(filtered.contains(&"決議採用 Rust 實作".to_string()));
        assert!(filtered.contains(&"Error: connection refused".to_string()));
        assert!(filtered.contains(&"完成代碼審查".to_string()));
    }

    #[test]
    fn test_qa_and_chitchat_filter() {
        let filter = NoiseFilter::new();
        assert!(!filter.is_signal_line("請問這個功能怎麼配置？"));
        assert!(!filter.is_signal_line("Q: 如何使用？"));
        assert!(!filter.is_signal_line("為什麼測試沒通過？"));
        assert!(!filter.is_signal_line("謝謝"));
        assert!(!filter.is_signal_line("感謝分享"));
        assert!(!filter.is_signal_line("沒問題"));
    }

    #[test]
    fn test_score_line_and_filter_with_scores() {
        let filter = NoiseFilter::new();
        assert!(filter.score_line("@Pollen 請實作模組 X") >= 2);
        assert!(filter.score_line("決議採用 Rust 實作") >= 2);
        assert!(filter.score_line("請問有什麼問題嗎？") < 0);

        let content = "決議採用 Rust 實作\n請問有什麼問題？\n@Pollen 請指派任務";
        let scored = filter.filter_lines_with_scores(content);
        assert_eq!(scored.len(), 2);
        assert_eq!(scored[0].0, "決議採用 Rust 實作");
        assert_eq!(scored[1].0, "@Pollen 請指派任務");
    }

    #[test]
    fn test_threshold_extreme_boundary() {
        // 當 threshold = i32::MAX 時，所有行皆被過濾
        let filter = NoiseFilter::with_threshold(i32::MAX);
        let content = "決議採用 Rust 實作\n@Pollen 請實作模組 X";
        let filtered = filter.filter_lines(content);
        assert!(filtered.is_empty());
    }
}
