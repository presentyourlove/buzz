//! TStack: 技巧堆疊調度器與輕量級執行鏈
//! 靈感來自 t3dotgg/tstack，為 Buzz 蜂群提供模組化技能熱插拔、線性/DAG 依賴調度與 Fallback 容錯

pub mod callable;
pub mod dispatcher;
pub mod registry;

pub use callable::{ClosureSkill, SkillCallable, SkillContext, SkillError, SkillInput, SkillOutput};
pub use dispatcher::{SkillStackDispatcher, StackExecutionResult, StackNode};
pub use registry::{SkillMetadata, SkillRegistry};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_linear_pipeline_execution() {
        let mut registry = SkillRegistry::new();

        // 技能 1: 抓取數據
        registry.register_skill(ClosureSkill::new("fetch_data", "Fetch Data", |_in, _ctx| {
            Ok(SkillOutput::success(json!({"raw_count": 42}), "數據抓取完成"))
        }));

        // 技能 2: 轉換處理
        registry.register_skill(ClosureSkill::new("transform_data", "Transform Data", |input, _ctx| {
            let prev = input.get_value("prev_result").unwrap();
            let count = prev["raw_count"].as_i64().unwrap();
            Ok(SkillOutput::success(json!({"processed_count": count * 2}), "數據加倍處理完成"))
        }));

        // 技能 3: 產出報告
        registry.register_skill(ClosureSkill::new("generate_report", "Generate Report", |input, _ctx| {
            let prev = input.get_value("prev_result").unwrap();
            let count = prev["processed_count"].as_i64().unwrap();
            Ok(SkillOutput::success(json!({"final_report": format!("總計共處理 {} 筆項目", count)}), "報告生成完畢"))
        }));

        let mut dispatcher = SkillStackDispatcher::new();
        dispatcher
            .add_linear_step("fetch_data", None)
            .add_linear_step("transform_data", None)
            .add_linear_step("generate_report", None);

        let mut ctx = SkillContext::new("Honey2");
        let result = dispatcher.execute(&registry, &SkillInput::new(), &mut ctx).unwrap();

        assert!(result.success);
        assert_eq!(result.executed_order, vec!["fetch_data", "transform_data", "generate_report"]);
        let final_val = result.final_output.unwrap().data;
        assert_eq!(final_val["final_report"], "總計共處理 84 筆項目");
    }

    #[test]
    fn test_dag_topology_dependency_order() {
        let mut registry = SkillRegistry::new();
        registry.register_skill(ClosureSkill::new("step_a", "Step A", |_in, _ctx| {
            Ok(SkillOutput::success(json!({"val": "A"}), "A 完成"))
        }));
        registry.register_skill(ClosureSkill::new("step_b", "Step B", |_in, _ctx| {
            Ok(SkillOutput::success(json!({"val": "B"}), "B 完成"))
        }));
        registry.register_skill(ClosureSkill::new("step_c", "Step C", |_in, _ctx| {
            Ok(SkillOutput::success(json!({"val": "C"}), "C 完成"))
        }));

        let mut dispatcher = SkillStackDispatcher::new();
        // C 依賴 B, B 依賴 A
        dispatcher.add_node(StackNode::new("step_c").with_dependency("step_b"));
        dispatcher.add_node(StackNode::new("step_b").with_dependency("step_a"));
        dispatcher.add_node(StackNode::new("step_a"));

        let mut ctx = SkillContext::new("Pollen");
        let result = dispatcher.execute(&registry, &SkillInput::new(), &mut ctx).unwrap();

        assert!(result.success);
        // 拓撲順序必須為 step_a -> step_b -> step_c
        assert_eq!(result.executed_order, vec!["step_a", "step_b", "step_c"]);
    }

    #[test]
    fn test_cycle_detection_guard() {
        let mut dispatcher = SkillStackDispatcher::new();
        // A 依賴 B, B 依賴 A (形成死結循環)
        dispatcher.add_node(StackNode::new("step_a").with_dependency("step_b"));
        dispatcher.add_node(StackNode::new("step_b").with_dependency("step_a"));

        let topology_res = dispatcher.resolve_topology();
        assert!(topology_res.is_err());
        match topology_res.unwrap_err() {
            SkillError::CycleDetected(msg) => {
                assert!(msg.contains("循環依賴"));
            }
            other => panic!("預期 CycleDetected 但得到 {:?}", other),
        }
    }

    #[test]
    fn test_fallback_recovery() {
        let mut registry = SkillRegistry::new();

        // 主要技能：必然報錯 (如遠端 API 伺服器掛掉)
        registry.register_skill(ClosureSkill::new("remote_api_query", "Remote API Query", |_in, _ctx| {
            Err(SkillError::ExecutionFailed("HTTP 503 遠端伺服器停機維護中".to_string()))
        }));

        // 備援技能：自動讀取本地快取
        registry.register_skill(ClosureSkill::new("local_cache_query", "Local Cache Query", |_in, _ctx| {
            Ok(SkillOutput::success(json!({"cached_data": "備援快取資料", "source": "offline_db"}), "本地快取載入成功"))
        }));

        let mut dispatcher = SkillStackDispatcher::new();
        dispatcher.add_node(StackNode::new("remote_api_query").with_fallback("local_cache_query"));

        let mut ctx = SkillContext::new("Sentry");
        let result = dispatcher.execute(&registry, &SkillInput::new(), &mut ctx).unwrap();

        assert!(result.success);
        assert_eq!(result.fallbacks_triggered, vec![("remote_api_query".to_string(), "local_cache_query".to_string())]);
        assert_eq!(result.final_output.unwrap().data["source"], "offline_db");
    }

    #[test]
    fn test_registry_hot_plug_and_tag_lookup() {
        let mut registry = SkillRegistry::new();

        let s1 = ClosureSkill::new("summarize_v1", "Summarizer V1", |_in, _ctx| {
            Ok(SkillOutput::success(json!({}), "ok"))
        }).with_tags(vec!["text".to_string(), "nlp".to_string()]);

        let s2 = ClosureSkill::new("summarize_v2", "Summarizer V2 (Hot Swap)", |_in, _ctx| {
            Ok(SkillOutput::success(json!({}), "ok"))
        }).with_tags(vec!["text".to_string(), "v2".to_string()]);

        registry.register_skill(s1);
        registry.register_skill(s2);

        assert_eq!(registry.len(), 2);

        // 按標籤檢索
        let text_skills = registry.find_by_tag("text");
        assert_eq!(text_skills.len(), 2);

        let v2_skills = registry.find_by_tag("v2");
        assert_eq!(v2_skills.len(), 1);
        assert_eq!(v2_skills[0].id(), "summarize_v2");

        // 熱插拔移除
        let removed = registry.unregister("summarize_v1");
        assert!(removed.is_some());
        assert_eq!(registry.len(), 1);
        assert!(!registry.contains("summarize_v1"));
    }

    #[test]
    fn test_condition_guard_skipping() {
        let mut registry = SkillRegistry::new();
        registry.register_skill(ClosureSkill::new("step_always", "Always Step", |_in, _ctx| {
            Ok(SkillOutput::success(json!({"flag": false}), "always"))
        }));
        registry.register_skill(ClosureSkill::new("step_skipped", "Skipped Step", |_in, _ctx| {
            Ok(SkillOutput::success(json!({"flag": true}), "skipped"))
        }));

        let mut dispatcher = SkillStackDispatcher::new();
        dispatcher.add_node(StackNode::new("step_always"));
        // 條件判斷：若 context 中的 enable_feature 為 false 則跳過
        dispatcher.add_node(
            StackNode::new("step_skipped")
                .with_dependency("step_always")
                .with_condition(|ctx, _outputs| {
                    ctx.shared_memory.get("enable_feature").and_then(|v| v.as_bool()).unwrap_or(false)
                }),
        );

        let mut ctx = SkillContext::new("Wax").with_memory("enable_feature", false);
        let result = dispatcher.execute(&registry, &SkillInput::new(), &mut ctx).unwrap();

        assert!(result.success);
        assert_eq!(result.executed_order, vec!["step_always"]);
        assert_eq!(result.skipped_steps, vec!["step_skipped"]);
    }
}

