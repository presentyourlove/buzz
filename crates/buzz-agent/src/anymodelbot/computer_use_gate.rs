//! AnyModelBot 方案 B：amb-computer-use-gate 桌面鍵鼠操作前置門禁
//!
//! 對 GUI 動作（按鍵序列、快捷鍵、滑鼠點擊座標）進行安全檢驗，
//! 阻斷高危破壞性按鍵與誤觸。

/// 鍵鼠操作門禁檢驗結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputerUseGateResult {
    /// 允許放行
    Allowed,
    /// 攔截阻斷
    Blocked(String),
}

/// 桌面操作安全門禁
#[derive(Debug, Default, Clone)]
pub struct ComputerUseGate;

impl ComputerUseGate {
    pub fn new() -> Self {
        Self
    }

    /// 檢驗鍵盤按鍵輸入安全性
    pub fn inspect_key_combination(&self, key_combination: &str) -> ComputerUseGateResult {
        let key_lower = key_combination.to_lowercase().replace(' ', "");

        // 高危快捷鍵黑名單
        let prohibited_combinations = [
            "ctrl+alt+del",
            "ctrl+alt+delete",
            "alt+f4",
            "win+l",
            "win+r",
            "ctrl+shift+esc",
        ];

        for combo in &prohibited_combinations {
            if key_lower == *combo || key_lower.contains(combo) {
                return ComputerUseGateResult::Blocked(format!(
                    "[amb-computer-use-gate] 門禁強制攔截：嚴禁發送系統級高危快捷鍵 ({})",
                    key_combination
                ));
            }
        }

        ComputerUseGateResult::Allowed
    }

    /// 檢驗滑鼠點擊座標合法性 (防負座標或異常超出主螢幕邊界之異常點擊)
    pub fn inspect_mouse_coordinates(&self, x: i32, y: i32, max_w: i32, max_h: i32) -> ComputerUseGateResult {
        if x < 0 || y < 0 || x > max_w || y > max_h {
            return ComputerUseGateResult::Blocked(format!(
                "[amb-computer-use-gate] 門禁攔截：滑鼠點擊座標超出螢幕安全邊界 (x: {}, y: {}, 邊界: {}x{})",
                x, y, max_w, max_h
            ));
        }

        ComputerUseGateResult::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computer_use_gate_high_risk_keys() {
        let gate = ComputerUseGate::new();

        // 攔截高危快捷鍵
        assert!(matches!(
            gate.inspect_key_combination("Ctrl + Alt + Del"),
            ComputerUseGateResult::Blocked(_)
        ));
        assert!(matches!(
            gate.inspect_key_combination("Alt+F4"),
            ComputerUseGateResult::Blocked(_)
        ));

        // 允許常規按鍵
        assert_eq!(
            gate.inspect_key_combination("Ctrl+C"),
            ComputerUseGateResult::Allowed
        );
        assert_eq!(
            gate.inspect_key_combination("Enter"),
            ComputerUseGateResult::Allowed
        );
    }

    #[test]
    fn test_computer_use_gate_coordinates() {
        let gate = ComputerUseGate::new();

        // 正常點擊放行
        assert_eq!(
            gate.inspect_mouse_coordinates(1920 / 2, 1080 / 2, 1920, 1080),
            ComputerUseGateResult::Allowed
        );

        // 負座標與越界攔截
        assert!(matches!(
            gate.inspect_mouse_coordinates(-10, 500, 1920, 1080),
            ComputerUseGateResult::Blocked(_)
        ));
        assert!(matches!(
            gate.inspect_mouse_coordinates(2000, 500, 1920, 1080),
            ComputerUseGateResult::Blocked(_)
        ));
    }
}

