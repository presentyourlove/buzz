//! Daily Digest Generator Module
//! 
//! Generates daily conversation digests from Buzz channels with intelligent filtering
//! and decision extraction capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub author: String,
    pub content: String,
    pub timestamp: i64,
    pub channel: String,
    pub tags: Vec<String>,
    pub reply_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyDigest {
    pub date: String,
    pub channel: String,
    pub messages: Vec<ConversationMessage>,
    pub filtered_messages: Vec<ConversationMessage>,
    pub decisions: Vec<String>,
    pub summary: String,
    pub key_topics: Vec<String>,
    pub participants: Vec<String>,
}

pub struct DigestGenerator {
    noise_filter: NoiseFilter,
    decision_extractor: DecisionExtractor,
    topic_analyzer: TopicAnalyzer,
}

impl DigestGenerator {
    pub fn new() -> Self {
        Self {
            noise_filter: NoiseFilter::new(),
            decision_extractor: DecisionExtractor::new(),
            topic_analyzer: TopicAnalyzer::new(),
        }
    }
    
    /// Generate a daily digest from conversation messages
    pub fn generate_digest(&self, messages: Vec<ConversationMessage>, date: &str, channel: &str) -> DailyDigest {
        let filtered_messages = self.noise_filter.filter_noise(messages.clone());
        let decisions = self.decision_extractor.extract_decisions(&filtered_messages);
        let key_topics = self.topic_analyzer.analyze_topics(&filtered_messages);
        let participants: Vec<String> = filtered_messages.iter()
            .map(|msg| msg.author.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let summary = self.generate_summary(&filtered_messages, &decisions);
        
        DailyDigest {
            date: date.to_string(),
            channel: channel.to_string(),
            messages,
            filtered_messages,
            decisions,
            summary,
            key_topics,
            participants,
        }
    }
    
    fn generate_summary(&self, messages: &[ConversationMessage], decisions: &[String]) -> String {
        let message_count = messages.len();
        let decision_count = decisions.len();
        format!("Today {} messages with {} key decisions made.", message_count, decision_count)
    }
}

struct NoiseFilter {
    spam_patterns: Vec<String>,
    irrelevant_keywords: Vec<String>,
}

impl NoiseFilter {
    fn new() -> Self {
        Self {
            spam_patterns: vec!["spam".to_string(), "advertisement".to_string()],
            irrelevant_keywords: vec!["random".to_string(), "test".to_string()],
        }
    }
    
    fn filter_noise(&self, messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
        messages.into_iter()
            .filter(|msg| !self.is_noise(&msg.content))
            .collect()
    }
    
    fn is_noise(&self, content: &str) -> bool {
        let lower_content = content.to_lowercase();
        self.spam_patterns.iter().any(|pattern| lower_content.contains(pattern)) ||
            self.irrelevant_keywords.iter().any(|keyword| lower_content.contains(keyword))
    }
}

struct DecisionExtractor {
    decision_indicators: Vec<String>,
}

impl DecisionExtractor {
    fn new() -> Self {
        Self {
            decision_indicators: vec![
                "decided".to_string(),
                "approved".to_string(),
                "implemented".to_string(),
                "agreed".to_string(),
                "committed".to_string(),
            ],
        }
    }
    
    fn extract_decisions(&self, messages: &[ConversationMessage]) -> Vec<String> {
        messages.iter()
            .filter_map(|msg| {
                let lower_content = msg.content.to_lowercase();
                for indicator in &self.decision_indicators {
                    if lower_content.contains(indicator) {
                        return Some(msg.content.clone());
                    }
                }
                None
            })
            .collect()
    }
}

struct TopicAnalyzer {
    topic_keywords: HashMap<String, Vec<String>>,
}

impl TopicAnalyzer {
    fn new() -> Self {
        let mut topic_keywords = HashMap::new();
        topic_keywords.insert("architecture".to_string(), vec!["architecture".to_string(), "design".to_string()]);
        topic_keywords.insert("performance".to_string(), vec!["performance".to_string(), "optimization".to_string()]);
        topic_keywords.insert("security".to_string(), vec!["security".to_string(), "vulnerability".to_string()]);
        Self { topic_keywords }
    }
    
    fn analyze_topics(&self, messages: &[ConversationMessage]) -> Vec<String> {
        let mut topic_counts: HashMap<String, usize> = HashMap::new();
        
        for msg in messages {
            let lower_content = msg.content.to_lowercase();
            for (topic, keywords) in &self.topic_keywords {
                let count = keywords.iter().filter(|keyword| lower_content.contains(keyword)).count();
                if count > 0 {
                    *topic_counts.entry(topic.clone()).or_insert(0) += count;
                }
            }
        }
        
        let mut topics: Vec<(String, usize)> = topic_counts.into_iter().collect();
        topics.sort_by(|a, b| b.1.cmp(&a.1));
        topics.into_iter().take(5).map(|(topic, _)| topic).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_digest_generation() {
        let generator = DigestGenerator::new();
        let messages = vec![
            ConversationMessage {
                id: "1".to_string(),
                author: "user1".to_string(),
                content: "We decided to implement the new feature".to_string(),
                timestamp: 1234567890,
                channel: "general".to_string(),
                tags: vec![],
                reply_to: None,
            },
            ConversationMessage {
                id: "2".to_string(),
                author: "user2".to_string(),
                content: "This is just a random test message".to_string(),
                timestamp: 1234567891,
                channel: "general".to_string(),
                tags: vec![],
                reply_to: None,
            },
        ];
        
        let digest = generator.generate_digest(messages, "2026-11-02", "general");
        assert_eq!(digest.decisions.len(), 1);
        assert_eq!(digest.decisions[0], "We decided to implement the new feature");
        assert_eq!(digest.filtered_messages.len(), 1);
    }
    
    #[test]
    fn test_noise_filtering() {
        let filter = NoiseFilter::new();
        let messages = vec![
            ConversationMessage {
                id: "1".to_string(),
                author: "user1".to_string(),
                content: "This is a normal message".to_string(),
                timestamp: 1234567890,
                channel: "general".to_string(),
                tags: vec![],
                reply_to: None,
            },
            ConversationMessage {
                id: "2".to_string(),
                author: "user2".to_string(),
                content: "This is spam advertisement".to_string(),
                timestamp: 1234567891,
                channel: "general".to_string(),
                tags: vec![],
                reply_to: None,
            },
        ];
        
        let filtered = filter.filter_noise(messages);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "This is a normal message");
    }
}
