//! 混合感知定位模組 (OmniParser 純視覺 + Native OS API 雙軌整合)

use serde::{Deserialize, Serialize};

/// 螢幕上偵測出的 UI 元素
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UIElement {
    pub id: usize,
    pub label: String,
    pub element_type: String, // "button", "input", "link", "icon", "text"
    pub bbox: [f32; 4],       // [ymin, xmin, ymax, xmax] 歸一化座標 (0.0 ~ 1.0)
    pub center_point: [u32; 2], // [x, y] 絕對螢幕像素座標
    pub confidence: f32,
    pub is_clickable: bool,
}

/// 感知來源策略
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PerceptionStrategy {
    /// 優先原生 OS Accessibility API (Windows UI Automation)
    NativeAccessibility,
    /// 純視覺 OmniParser 解析 (YOLOv9-E + Florence-2)
    PureVisionOmniParser,
    /// 雙軌融合 (Hybrid: Native 優先，缺失處由純視覺補齊)
    HybridFusion,
}

/// 視覺感知結果摘要
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerceptionResult {
    pub strategy_used: PerceptionStrategy,
    pub screen_width: u32,
    pub screen_height: u32,
    pub elements: Vec<UIElement>,
    pub latency_ms: u64,
}

pub struct VisionPerceptionTool {
    pub proxy_endpoint: String,
    pub default_strategy: PerceptionStrategy,
}

impl Default for VisionPerceptionTool {
    fn default() -> Self {
        Self {
            proxy_endpoint: "http://127.0.0.1:8989/parse_screen".to_string(),
            default_strategy: PerceptionStrategy::HybridFusion,
        }
    }
}

impl VisionPerceptionTool {
    pub fn new(endpoint: &str, strategy: PerceptionStrategy) -> Self {
        Self {
            proxy_endpoint: endpoint.to_string(),
            default_strategy: strategy,
        }
    }

    /// 解析螢幕影像並提取可交互元素
    pub fn parse_screen_mock(&self, width: u32, height: u32) -> PerceptionResult {
        // 內建確定性預設元素（供單元測試與離線降級）
        let elements = vec![
            UIElement {
                id: 1,
                label: "Start Agent Button".to_string(),
                element_type: "button".to_string(),
                bbox: [0.1, 0.1, 0.15, 0.25],
                center_point: [(width as f32 * 0.175) as u32, (height as f32 * 0.125) as u32],
                confidence: 0.96,
                is_clickable: true,
            },
            UIElement {
                id: 2,
                label: "Chat Input Field".to_string(),
                element_type: "input".to_string(),
                bbox: [0.85, 0.1, 0.95, 0.9],
                center_point: [(width as f32 * 0.5) as u32, (height as f32 * 0.9) as u32],
                confidence: 0.98,
                is_clickable: true,
            },
        ];

        PerceptionResult {
            strategy_used: self.default_strategy,
            screen_width: width,
            screen_height: height,
            elements,
            latency_ms: 45,
        }
    }
}

