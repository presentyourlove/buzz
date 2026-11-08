//! TStack: SkillRegistry 技巧動態註冊表與熱插拔管理器

use crate::tstack::callable::SkillCallable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 技能元數據 (用於列表展示與 Agent 認知檢索)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

/// 技巧動態註冊表
pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn SkillCallable>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// 註冊新技能（支援動態熱插拔與覆蓋）
    pub fn register(&mut self, skill: Box<dyn SkillCallable>) {
        let id = skill.id().to_string();
        self.skills.insert(id, skill);
    }

    /// 便捷註冊泛型技巧實例
    pub fn register_skill<S: SkillCallable + 'static>(&mut self, skill: S) {
        self.register(Box::new(skill));
    }

    /// 熱插拔移除技能
    pub fn unregister(&mut self, id: &str) -> Option<Box<dyn SkillCallable>> {
        self.skills.remove(id)
    }

    /// 根據 ID 查詢技能
    pub fn get(&self, id: &str) -> Option<&dyn SkillCallable> {
        self.skills.get(id).map(|s| s.as_ref())
    }

    /// 檢查技能是否已註冊
    pub fn contains(&self, id: &str) -> bool {
        self.skills.contains_key(id)
    }

    /// 已註冊技能數量
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// 根據分類標籤檢索技能
    pub fn find_by_tag(&self, tag: &str) -> Vec<&dyn SkillCallable> {
        self.skills
            .values()
            .filter(|s| s.tags().iter().any(|t| t == tag))
            .map(|s| s.as_ref())
            .collect()
    }

    /// 獲取所有已註冊技能之元數據列表
    pub fn list_metadata(&self) -> Vec<SkillMetadata> {
        let mut list: Vec<SkillMetadata> = self
            .skills
            .values()
            .map(|s| SkillMetadata {
                id: s.id().to_string(),
                name: s.name().to_string(),
                description: s.description().to_string(),
                tags: s.tags(),
            })
            .collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

