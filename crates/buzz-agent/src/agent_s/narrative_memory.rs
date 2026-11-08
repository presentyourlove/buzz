//! Agent-S 敘事長期記憶引擎 (NarrativeMemory)
//!
//! 將特工執行歷史抽象為「情境-行動-結果」(Context-Action-Result, CAR) 三元組，
//! 支援標籤檢索、相似度打分與低置信度自修剪。

use serde::{Deserialize, Serialize};

/// 任務執行成果列舉
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpisodeOutcome {
    Success,
    Failure,
    Partial,
}

impl std::fmt::Display for EpisodeOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpisodeOutcome::Success => write!(f, "成功 (Success)"),
            EpisodeOutcome::Failure => write!(f, "失敗 (Failure)"),
            EpisodeOutcome::Partial => write!(f, "部分成功 (Partial)"),
        }
    }
}

/// 敘事長期記憶片段 (CAR 三元組)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NarrativeEpisode {
    pub id: String,
    pub context: String,
    pub action_summary: String,
    pub result: String,
    pub outcome: EpisodeOutcome,
    pub tags: Vec<String>,
    pub confidence_score: f32,
    pub recorded_at: u64,
}

impl NarrativeEpisode {
    /// 建立新的敘事記憶片段
    pub fn new(
        id: impl Into<String>,
        context: impl Into<String>,
        action_summary: impl Into<String>,
        result: impl Into<String>,
        outcome: EpisodeOutcome,
        tags: Vec<String>,
        confidence_score: f32,
        recorded_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            context: context.into(),
            action_summary: action_summary.into(),
            result: result.into(),
            outcome,
            tags,
            confidence_score: confidence_score.clamp(0.0, 1.0),
            recorded_at,
        }
    }
}

/// 敘事記憶管理器
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NarrativeMemory {
    episodes: Vec<NarrativeEpisode>,
}

impl NarrativeMemory {
    pub fn new() -> Self {
        Self {
            episodes: Vec::new(),
        }
    }

    /// 加入一筆新敘事記憶
    pub fn add_episode(&mut self, episode: NarrativeEpisode) -> String {
        let id = episode.id.clone();
        // 若存在相同 ID 則覆蓋，否則新增
        if let Some(pos) = self.episodes.iter().position(|e| e.id == id) {
            self.episodes[pos] = episode;
        } else {
            self.episodes.push(episode);
        }
        id
    }

    /// 根據查詢字串與標籤檢索相關記憶
    pub fn search_episodes(&self, query: &str, limit: usize) -> Vec<&NarrativeEpisode> {
        let query_lower = query.to_lowercase();
        let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(&NarrativeEpisode, f32)> = self
            .episodes
            .iter()
            .map(|ep| {
                let mut score = 0.0f32;

                // 標籤完全匹配權重最高
                for tag in &ep.tags {
                    let tag_lower = tag.to_lowercase();
                    if query_lower.contains(&tag_lower) {
                        score += 3.0;
                    }
                }

                // 情境包含關鍵字
                let ctx_lower = ep.context.to_lowercase();
                for token in &query_tokens {
                    if ctx_lower.contains(token) {
                        score += 1.5;
                    }
                }

                // 行動摘要包含關鍵字
                let act_lower = ep.action_summary.to_lowercase();
                for token in &query_tokens {
                    if act_lower.contains(token) {
                        score += 1.0;
                    }
                }

                // 成功案例額外加成
                if ep.outcome == EpisodeOutcome::Success {
                    score += 0.5;
                }

                // 結合自身置信度
                score *= ep.confidence_score;

                (ep, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        // 依評分由大到小排序
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(limit).map(|(ep, _)| ep).collect()
    }

    /// 修剪置信度低於門檻之舊記憶
    pub fn prune_low_confidence(&mut self, threshold: f32) -> usize {
        let initial_len = self.episodes.len();
        self.episodes.retain(|ep| ep.confidence_score >= threshold);
        initial_len - self.episodes.len()
    }

    /// 取得記憶筆數
    pub fn count(&self) -> usize {
        self.episodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_narrative_memory_store_and_search() {
        let mut memory = NarrativeMemory::new();

        let ep1 = NarrativeEpisode::new(
            "ep_rust_01",
            "Windows MinGW 環境編譯 buzz-agent 時出現 ld 路徑錯誤",
            "設置 CARGO_TARGET_DIR=C:\\buzz-target 並配置 AWS_LC_SYS_NO_ASM=1 避開 MinGW 中文字元路徑 bug",
            "編譯與測試 100% 成功通過",
            EpisodeOutcome::Success,
            vec!["rust".into(), "windows".into(), "compile".into()],
            0.95,
            1794120000,
        );

        let ep2 = NarrativeEpisode::new(
            "ep_nostr_01",
            "NIP-ER 定時提醒事件未被 Relay 正確過濾",
            "在 subscription filter 中加入 kind:30300 與 d-tag 限制",
            "定時喚醒正常觸發",
            EpisodeOutcome::Success,
            vec!["nostr".into(), "reminder".into(), "nip-er".into()],
            0.88,
            1794121000,
        );

        memory.add_episode(ep1);
        memory.add_episode(ep2);

        assert_eq!(memory.count(), 2);

        // 檢索 rust 編譯相關
        let results = memory.search_episodes("Windows rust 編譯報錯", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "ep_rust_01");

        // 檢索 nostr 定時提醒
        let results_nostr = memory.search_episodes("nip-er 定時提醒", 5);
        assert!(!results_nostr.is_empty());
        assert_eq!(results_nostr[0].id, "ep_nostr_01");
    }

    #[test]
    fn test_narrative_memory_prune() {
        let mut memory = NarrativeMemory::new();

        memory.add_episode(NarrativeEpisode::new(
            "low_conf",
            "未知網路超時",
            "重試一次",
            "依然失敗",
            EpisodeOutcome::Failure,
            vec!["network".into()],
            0.2,
            1794120000,
        ));

        memory.add_episode(NarrativeEpisode::new(
            "high_conf",
            "正確配置代理",
            "設定 HTTP_PROXY",
            "連線成功",
            EpisodeOutcome::Success,
            vec!["network".into()],
            0.9,
            1794120000,
        ));

        assert_eq!(memory.count(), 2);
        let pruned = memory.prune_low_confidence(0.5);
        assert_eq!(pruned, 1);
        assert_eq!(memory.count(), 1);
    }
}

