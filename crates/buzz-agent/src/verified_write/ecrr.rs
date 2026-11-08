//! Explore-Compile-Run-Repair (ECRR) 自動修復工作流引擎
//! 
//! 特性：
//! 1. 四階段狀態機：Explore (探索分析) -> Compile (靜態編譯預檢) -> Run (單元測試運行) -> Repair (錯誤修復)。
//! 2. 最大修復次數熔斷保護 (Max Repairs Guard)，防止特工陷入修復死循環。
//! 3. 完整執行軌跡記錄，便於事後稽核與經驗抽取。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum EcrrState {
    Explore,
    Compile,
    Run,
    Repair,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EcrrReceipt {
    pub task_id: String,
    pub total_attempts: usize,
    pub final_state: EcrrState,
    pub trace_log: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExploreCompileRunRepair {
    pub task_id: String,
    pub max_repairs: usize,
    pub current_repair_count: usize,
    pub state: EcrrState,
    pub trace_log: Vec<String>,
}

impl ExploreCompileRunRepair {
    pub fn new(task_id: &str, max_repairs: usize) -> Self {
        Self {
            task_id: task_id.to_string(),
            max_repairs,
            current_repair_count: 0,
            state: EcrrState::Explore,
            trace_log: vec![format!("[ECRR] 任務 {task_id} 啟動，進入 Explore 階段。")],
        }
    }

    /// 推進 ECRR 狀態機
    /// compile_ok: 靜態編譯或語法預檢是否通過
    /// test_ok: 單元測試或功能驗收是否通過
    /// error_message: 失敗時的具體錯誤堆疊
    pub fn step(&mut self, compile_ok: bool, test_ok: bool, error_message: Option<&str>) -> EcrrState {
        match self.state {
            EcrrState::Explore => {
                self.trace_log.push("[Explore] 完成目標檔案結構與符號分析，推進至 Compile 階段。".into());
                self.state = EcrrState::Compile;
            }
            EcrrState::Compile => {
                if compile_ok {
                    self.trace_log.push("[Compile] 靜態編譯與預檢通過，推進至 Run 階段。".into());
                    self.state = EcrrState::Run;
                } else {
                    let err = error_message.unwrap_or("未知名編譯錯誤");
                    self.trace_log.push(format!("[Compile 失敗] 觸發修復: {err}"));
                    self.transition_to_repair();
                }
            }
            EcrrState::Run => {
                if test_ok {
                    self.trace_log.push("[Run] 單元測試驗證 100% 通過，工作流成功閉環。".into());
                    self.state = EcrrState::Succeeded;
                } else {
                    let err = error_message.unwrap_or("單元測試未通過");
                    self.trace_log.push(format!("[Run 失敗] 觸發修復: {err}"));
                    self.transition_to_repair();
                }
            }
            EcrrState::Repair => {
                self.trace_log.push(format!(
                    "[Repair] 完成第 {} 次修復代碼調整，重新進入 Compile 階段。",
                    self.current_repair_count
                ));
                self.state = EcrrState::Compile;
            }
            EcrrState::Succeeded | EcrrState::Failed => {}
        }

        self.state
    }

    fn transition_to_repair(&mut self) {
        if self.current_repair_count >= self.max_repairs {
            self.trace_log.push(format!(
                "[熔斷] 修復次數達到上限 ({})，安全中止工作流。",
                self.max_repairs
            ));
            self.state = EcrrState::Failed;
        } else {
            self.current_repair_count += 1;
            self.state = EcrrState::Repair;
        }
    }

    /// 取得最終收據
    pub fn finish(self) -> EcrrReceipt {
        EcrrReceipt {
            task_id: self.task_id,
            total_attempts: self.current_repair_count,
            final_state: self.state,
            trace_log: self.trace_log,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecrr_smooth_success() {
        let mut ecrr = ExploreCompileRunRepair::new("task_smooth", 3);
        assert_eq!(ecrr.step(true, true, None), EcrrState::Compile);
        assert_eq!(ecrr.step(true, true, None), EcrrState::Run);
        assert_eq!(ecrr.step(true, true, None), EcrrState::Succeeded);

        let receipt = ecrr.finish();
        assert_eq!(receipt.final_state, EcrrState::Succeeded);
        assert_eq!(receipt.total_attempts, 0);
    }

    #[test]
    fn test_ecrr_repair_and_recover() {
        let mut ecrr = ExploreCompileRunRepair::new("task_repair", 3);
        ecrr.step(true, true, None); // to Compile
        // Compile fails
        assert_eq!(ecrr.step(false, false, Some("syntax error")), EcrrState::Repair);
        assert_eq!(ecrr.current_repair_count, 1);
        // Repair transitions back to Compile
        assert_eq!(ecrr.step(true, true, None), EcrrState::Compile);
        // Now compile passes -> Run
        assert_eq!(ecrr.step(true, true, None), EcrrState::Run);
        // Test passes -> Succeeded
        assert_eq!(ecrr.step(true, true, None), EcrrState::Succeeded);
    }

    #[test]
    fn test_ecrr_max_repairs_fuse() {
        let mut ecrr = ExploreCompileRunRepair::new("task_fuse", 2);
        ecrr.step(true, true, None); // to Compile
        ecrr.step(false, false, Some("err1")); // to Repair (count=1)
        ecrr.step(true, true, None); // to Compile
        ecrr.step(false, false, Some("err2")); // to Repair (count=2)
        ecrr.step(true, true, None); // to Compile
        // 3rd failure reaches max -> Failed
        assert_eq!(ecrr.step(false, false, Some("err3")), EcrrState::Failed);
    }
}

