//! Noise Filter Module
//! 
//! Filters out noise, spam, and irrelevant messages from conversation data
//! to improve digest quality and relevance.

use std::collections::HashSet;

#[derive(Debug)]
pub struct NoiseFilter {
    spam_patterns: HashSet<String>,
    irrelevant_keywords: HashSet<String>,
    noise_threshold: f32,
    min_message_length: usize,
    profanity_list: HashSet<String>,
}

impl NoiseFilter {
    pub fn new() -> Self {
        let mut spam_patterns = HashSet::new();
        spam_patterns.insert("spam".to_string());
        spam_patterns.insert("advertisement".to_string());
        spam_patterns.insert("promote".to_string());
        spam_patterns.insert("buy now".to_string());
        spam_patterns.insert("click here".to_string());
        
        let mut irrelevant_keywords = HashSet::new();
        irrelevant_keywords.insert("random".to_string());
        irrelevant_keywords.insert("test".to_string());
        irrelevant_keywords.insert("demo".to_string());
        irrelevant_keywords.insert("sample".to_string());
        irrelevant_keywords.insert("example".to_string());
        
        let mut profanity_list = HashSet::new();
        profanity_list.insert("spam".to_string());
        profanity_list.insert("badword1".to_string());
        profanity_list.insert("badword2".to_string());
        
        Self {
            spam_patterns,
            irrelevant_keywords,
            noise_threshold: 0.8,
            min_message_length: 10,
            profanity_list,
        }
    }
    
    /// Filter noise from a collection of messages
    pub fn filter_messages(&self, messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
        messages.into_iter()
            .filter(|msg| self.is_clean_message(&msg.content))
            .filter(|msg| self.is_meaningful_content(&msg.content))
            .filter(|msg| !self.is_spam(&msg.content))
            .collect()
    }
    
    /// Determine if a message contains noise
    fn is_clean_message(&self, content: &str) -> bool {
        let lower_content = content.to_lowercase();
        !(self.spam_patterns.iter().any(|pattern| lower_content.contains(pattern)) ||
            self.irrelevant_keywords.iter().any(|keyword| lower_content.contains(keyword)) ||
            self.profanity_list.iter().any(|word| lower_content.contains(word)))
    }
    
    /// Check if message length is meaningful
    fn is_meaningful_content(&self, content: &str) -> bool {
        content.len() >= self.min_message_length
    }
    
    /// Check if content is spam-like
    fn is_spam(&self, content: &str) -> bool {
        let lower_content = content.to_lowercase();
        let spam_score = self.calculate_spam_score(&content);
        spam_score > self.noise_threshold
    }
    
    /// Calculate spam score based on content analysis
    fn calculate_spam_score(&self, content: &str) -> f32 {
        let lower_content = content.to_lowercase();
        content.len() as f32 *
            (self.spam_patterns.iter()
                .filter(|pattern| lower_content.contains(pattern))
                .count() as f32) /
            self.spam_patterns.len() as f32
    }
    
    /// Get filter statistics
    pub fn get_filter_stats(&self, original: &[ConversationMessage], filtered: &[ConversationMessage]) -> FilterStats {
        FilterStats {
            original_count: original.len(),
            filtered_count: filtered.len(),
            removal_percentage: ((original.len() - filtered.len()) as f32 / original.len() as f32) * 100.0,
            spam_patterns_matched: self.spam_patterns.iter().filter(|pattern| {
                original.iter().any(|msg| msg.content.to_lowercase().contains(pattern))
            }).count(),
        }
    }
}

#[derive(Debug)]
pub struct ConversationMessage {
    pub id: String,
    pub author: String,
    pub content: String,
    pub timestamp: i64,
    pub channel: String,
    pub tags: Vec<String>,
    pub reply_to: Option<String>,
}

#[derive(Debug)]
pub struct FilterStats {
    pub original_count: usize,
    pub filtered_count: usize,
    pub removal_percentage: f32,
    pub spam_patterns_matched: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_noise_filter_basic() {
        let filter = NoiseFilter::new();
        let messages = vec![
            ConversationMessage {
                id: "1".to_string(),
                author: "user1".to_string(),
                content: "This is a normal meaningful message".to_string(),
                timestamp: 1234567890,
                channel: "general".to_string(),
                tags: vec![],
                reply_to: None,
            },
            ConversationMessage {
                id: "2".to_string(),
                author: "user2".to_string(),
                content: "Buy now spam advertisement".to_string(),
                timestamp: 1234567891,
                channel: "general".to_string(),
                tags: vec![],
                reply_to: None,
            },
            ConversationMessage {
                id: "3".to_string(),
                author: "user3".to_string(),
                content: "Short".to_string(),
                timestamp: 1234567892,
                channel: "general".to_string(),
                tags: vec![],
                reply_to: None,
            },
        ];
        
        let filtered = filter.filter_messages(messages);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "This is a normal meaningful message");
    }
    
    #[test]
    fn test_spam_detection() {
        let filter = NoiseFilter::new();
        let spam_content = "Buy now! Click here for amazing offers!";
        assert!(filter.is_spam(spam_content));
        
        let normal_content = "This is a normal discussion about architecture.";
        assert!(!filter.is_spam(normal_content));
    }
    
    #[test]
    fn test_meaningful_content() {
        let filter = NoiseFilter::new();
        let short_content = "Hi";
        assert!(!filter.is_meaningful_content(short_content));
        
        let long_content = "This is a very long meaningful message that contains useful information for discussion and decision making.";
        assert!(filter.is_meaningful_content(long_content));
    }
}
