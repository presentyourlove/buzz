//! Agent-S 雙層認知架構核心 (Agent-S Dual-Layer Cognitive Architecture)
//!
//! 包含三大支柱：
//! 1. `narrative_memory`：敘事長期記憶 (CAR 三元組、經驗檢索與自修剪)
//! 2. `router`：特工動態意圖路由決策器 (專家領域矩陣匹配)
//! 3. `skill_synthesizer`：技能動態合成管線與安全防爆門禁

pub mod narrative_memory;
pub mod router;
pub mod skill_synthesizer;

pub use narrative_memory::{EpisodeOutcome, NarrativeEpisode, NarrativeMemory};
pub use router::{AgentRouter, RoutingDecision, SpecialistRole};
pub use skill_synthesizer::{SkillSynthesisError, SkillSynthesizer, SynthesizedSkill};

