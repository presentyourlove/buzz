//! TStack: SkillStackDispatcher 技巧堆疊調度器
//! 支援線性管線與 DAG 拓撲依賴排序、循環檢測、條件跳轉與失敗自動 Fallback 回退

use crate::tstack::callable::{SkillContext, SkillError, SkillInput, SkillOutput};
use crate::tstack::registry::SkillRegistry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

/// 堆疊執行單一步驟節點
pub struct StackNode {
    /// 目標技能 ID
    pub skill_id: String,
    /// 前置依賴之技能 ID 清單
    pub dependencies: Vec<String>,
    /// 若執行失敗時自動導流之備援技能 ID
    pub fallback_skill_id: Option<String>,
    /// 客製化輸入覆蓋（若無則傳遞上一步之輸出或初始輸入）
    pub custom_input: Option<SkillInput>,
    /// 是否可忽略失敗繼續推進後續節點
    pub continue_on_error: bool,
    /// 動態條件閘門：若回傳 false 則跳過本步驟
    pub condition: Option<Arc<dyn Fn(&SkillContext, &HashMap<String, SkillOutput>) -> bool + Send + Sync>>,
}

impl StackNode {
    pub fn new(skill_id: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            dependencies: Vec::new(),
            fallback_skill_id: None,
            custom_input: None,
            continue_on_error: false,
            condition: None,
        }
    }

    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    pub fn with_fallback(mut self, fallback_id: impl Into<String>) -> Self {
        self.fallback_skill_id = Some(fallback_id.into());
        self
    }

    pub fn with_custom_input(mut self, input: SkillInput) -> Self {
        self.custom_input = Some(input);
        self
    }

    pub fn continue_on_error(mut self, cont: bool) -> Self {
        self.continue_on_error = cont;
        self
    }

    pub fn with_condition<F>(mut self, cond: F) -> Self
    where
        F: Fn(&SkillContext, &HashMap<String, SkillOutput>) -> bool + Send + Sync + 'static,
    {
        self.condition = Some(Arc::new(cond));
        self
    }
}

/// 技巧堆疊全流程執行報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackExecutionResult {
    pub success: bool,
    pub step_outputs: HashMap<String, SkillOutput>,
    pub final_output: Option<SkillOutput>,
    pub executed_order: Vec<String>,
    pub fallbacks_triggered: Vec<(String, String)>,
    pub skipped_steps: Vec<String>,
    pub total_duration_ms: u64,
}

/// TStack 技巧堆疊調度器
pub struct SkillStackDispatcher {
    nodes: Vec<StackNode>,
}

impl SkillStackDispatcher {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// 線性管線輔助建構：Step 1 -> Step 2 -> Step 3
    pub fn add_linear_step(&mut self, skill_id: impl Into<String>, fallback_id: Option<String>) -> &mut Self {
        let skill_id_str = skill_id.into();
        let mut node = StackNode::new(skill_id_str.clone());
        if let Some(fb) = fallback_id {
            node = node.with_fallback(fb);
        }
        if let Some(prev) = self.nodes.last() {
            node = node.with_dependency(prev.skill_id.clone());
        }
        self.nodes.push(node);
        self
    }

    /// 加入任意帶依賴的 DAG 節點
    pub fn add_node(&mut self, node: StackNode) -> &mut Self {
        self.nodes.push(node);
        self
    }

    /// 使用 Kahn's 演算法進行拓撲排序並檢測循環依賴
    pub fn resolve_topology(&self) -> Result<Vec<usize>, SkillError> {
        let n = self.nodes.len();
        if n == 0 {
            return Ok(Vec::new());
        }

        let mut id_to_idx = HashMap::new();
        for (idx, node) in self.nodes.iter().enumerate() {
            id_to_idx.insert(node.skill_id.as_str(), idx);
        }

        let mut in_degree = vec![0; n];
        let mut adj = vec![Vec::new(); n];

        for (idx, node) in self.nodes.iter().enumerate() {
            for dep in &node.dependencies {
                if let Some(&dep_idx) = id_to_idx.get(dep.as_str()) {
                    adj[dep_idx].push(idx);
                    in_degree[idx] += 1;
                }
            }
        }

        let mut queue = VecDeque::new();
        for (i, &deg) in in_degree.iter().enumerate() {
            if deg == 0 {
                queue.push_back(i);
            }
        }

        let mut sorted = Vec::new();
        while let Some(u) = queue.pop_front() {
            sorted.push(u);
            for &v in &adj[u] {
                in_degree[v] -= 1;
                if in_degree[v] == 0 {
                    queue.push_back(v);
                }
            }
        }

        if sorted.len() != n {
            return Err(SkillError::CycleDetected(
                "循環依賴檢測到死結：節點無法在無環有向圖中解析拓撲順序".to_string(),
            ));
        }

        Ok(sorted)
    }

    /// 執行堆疊調度
    pub fn execute(
        &self,
        registry: &SkillRegistry,
        initial_input: &SkillInput,
        ctx: &mut SkillContext,
    ) -> Result<StackExecutionResult, SkillError> {
        let start_time = Instant::now();
        let sorted_indices = self.resolve_topology()?;

        let mut step_outputs: HashMap<String, SkillOutput> = HashMap::new();
        let mut executed_order = Vec::new();
        let mut fallbacks_triggered = Vec::new();
        let mut skipped_steps = Vec::new();
        let mut last_output: Option<SkillOutput> = None;

        for &idx in &sorted_indices {
            let node = &self.nodes[idx];

            // 1. 條件檢查 (Condition Guard)
            if let Some(cond) = &node.condition {
                if !cond(ctx, &step_outputs) {
                    skipped_steps.push(node.skill_id.clone());
                    continue;
                }
            }

            // 2. 前置依賴驗證
            for dep in &node.dependencies {
                if let Some(dep_out) = step_outputs.get(dep) {
                    if !dep_out.success && !node.continue_on_error {
                        return Err(SkillError::DependencyMissing(format!(
                            "前置技能 {} 執行失敗，無法推進至 {}",
                            dep, node.skill_id
                        )));
                    }
                }
            }

            // 3. 輸入組裝 (優先使用自定義輸入，否則繼承前一步輸出或初始輸入)
            let current_input = if let Some(custom) = &node.custom_input {
                custom.clone()
            } else if let Some(prev_out) = &last_output {
                SkillInput::new()
                    .with_param("prev_result", prev_out.data.clone())
                    .with_payload(prev_out.message.clone())
            } else {
                initial_input.clone()
            };

            // 4. 調用主要技能
            let skill_res = match registry.get(&node.skill_id) {
                Some(skill) => skill.execute(&current_input, ctx),
                None => Err(SkillError::Internal(format!("技能未註冊: {}", node.skill_id))),
            };

            // 5. 處理結果與自動 Fallback
            let mut final_node_output = match skill_res {
                Ok(out) if out.success => {
                    executed_order.push(node.skill_id.clone());
                    out
                }
                err => {
                    let err_msg = match err {
                        Ok(failed_out) => failed_out.message,
                        Err(e) => e.to_string(),
                    };

                    // 檢查是否有 Fallback
                    if let Some(fb_id) = &node.fallback_skill_id {
                        if let Some(fb_skill) = registry.get(fb_id) {
                            let fb_input = current_input
                                .clone()
                                .with_param("fallback_reason", Value::String(err_msg.clone()));
                            match fb_skill.execute(&fb_input, ctx) {
                                Ok(fb_out) => {
                                    fallbacks_triggered.push((node.skill_id.clone(), fb_id.clone()));
                                    executed_order.push(format!("{}(Fallback:{})", node.skill_id, fb_id));
                                    fb_out
                                }
                                Err(fb_err) => {
                                    if node.continue_on_error {
                                        SkillOutput::failure(format!("Fallback 亦失敗: {}", fb_err))
                                    } else {
                                        return Err(SkillError::ExecutionFailed(format!(
                                            "主要技能與 Fallback 技能皆失敗: {} -> {}",
                                            err_msg, fb_err
                                        )));
                                    }
                                }
                            }
                        } else if node.continue_on_error {
                            SkillOutput::failure(format!("主要技能失敗且未找到 Fallback 技能: {}", err_msg))
                        } else {
                            return Err(SkillError::ExecutionFailed(format!(
                                "主要技能失敗且備援不存在: {}",
                                err_msg
                            )));
                        }
                    } else if node.continue_on_error {
                        executed_order.push(format!("{}(Failed)", node.skill_id));
                        SkillOutput::failure(err_msg)
                    } else {
                        return Err(SkillError::ExecutionFailed(err_msg));
                    }
                }
            };

            final_node_output.execution_ms = start_time.elapsed().as_millis() as u64;
            last_output = Some(final_node_output.clone());
            step_outputs.insert(node.skill_id.clone(), final_node_output);
        }

        let total_duration_ms = start_time.elapsed().as_millis() as u64;
        let all_success = step_outputs.values().all(|o| o.success);

        Ok(StackExecutionResult {
            success: all_success,
            step_outputs,
            final_output: last_output,
            executed_order,
            fallbacks_triggered,
            skipped_steps,
            total_duration_ms,
        })
    }
}

impl Default for SkillStackDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

