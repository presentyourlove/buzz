//! Markdown Renderer Module
//! 
//! Handles conversion of parsed conversation data to formatted Markdown for Obsidian vault integration.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    pub template: String,
    pub output_dir: String,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            template: "# Daily Digest\n\nDate: {date}\nChannel: {channel}\n\n## Decisions\n{decisions}\n\n## Key Topics\n{topics}\n\n## Participants\n{participants}\n\n## Messages\n{messages}".to_string(),
            output_dir: "daily_digests".to_string(),
        }
    }
    
    /// Render daily digest data to Markdown format
    pub fn render_to_markdown(&self, digest_data: &DailyDigest) -> String {
        let mut decisions_section = String::new();
        for decision in &digest_data.decisions {
            decisions_section.push_str(&format!("\n- {}", decision));
        }
        
        let mut topics_section = String::new();
        for topic in &digest_data.key_topics {
            topics_section.push_str(&format!("\n- {}", topic));
        }
        
        let mut participants_section = String::new();
        for participant in &digest_data.participants {
            participants_section.push_str(&format!("\n- {}", participant));
        }
        
        let mut messages_section = String::new();
        for (i, message) in digest_data.filtered_messages.iter().enumerate() {
            messages_section.push_str(&format!("\n### Message {} (by {})", i + 1, message.author));
            messages_section.push_str(&format!("\n**Time:** {}", message.timestamp));
            messages_section.push_str(&format!("\n\n{}", message.content));
            messages_section.push_str("\n---\n");
        }
        
        let rendered = self.template
            .replace("{date}", &digest_data.date)
            .replace("{channel}", &digest_data.channel)
            .replace("{decisions}", &decisions_section)
            .replace("{topics}", &topics_section)
            .replace("{participants}", &participants_section)
            .replace("{messages}", &messages_section);
        
        rendered
    }
    
    /// Save Markdown content to file
    pub fn save_to_file(&self, content: &str, date: &str, channel: &str) -> Result<(), Box<dyn std::error::Error>> {
        let filename = format!("{}_{}.md", date.replace("/", "-").replace(" ", "_"), channel.replace(" ", "_"));
        let filepath = Path::new(&self.output_dir).join(filename);
        
        // Create directory if it doesn't exist
        if let Some(parent) = filepath.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(filepath, content)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub id: String,
    pub author: String,
    pub content: String,
    pub timestamp: i64,
    pub channel: String,
    pub tags: Vec<String>,
    pub reply_to: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_markdown_rendering() {
        let renderer = MarkdownRenderer::new();
        let digest = DailyDigest {
            date: "2026-11-02".to_string(),
            channel: "general".to_string(),
            messages: vec![],
            filtered_messages: vec![],
            decisions: vec!["Decided to implement feature".to_string()],
            summary: "Today 5 messages".to_string(),
            key_topics: vec!["architecture".to_string(), "performance".to_string()],
            participants: vec!["user1".to_string(), "user2".to_string()],
        };
        
        let markdown = renderer.render_to_markdown(&digest);
        assert!(markdown.contains("# Daily Digest"));
        assert!(markdown.contains("2026-11-02"));
        assert!(markdown.contains("general"));
        assert!(markdown.contains("Decided to implement feature"));
        assert!(markdown.contains("architecture"));
    }
    
    #[test]
    fn test_file_saving() {
        let renderer = MarkdownRenderer::new();
        let digest = DailyDigest {
            date: "2026-11-02".to_string(),
            channel: "test".to_string(),
            messages: vec![],
            filtered_messages: vec![],
            decisions: vec![],
            summary: "Test".to_string(),
            key_topics: vec![],
            participants: vec![],
        };
        
        let markdown = renderer.render_to_markdown(&digest);
        let result = renderer.save_to_file(&markdown, "2026-11-02", "test");
        assert!(result.is_ok());
        
        let filepath = Path::new("daily_digests").join("2026-11-02_test.md");
        assert!(filepath.exists());
        fs::remove_file(filepath).unwrap();
    }
}
