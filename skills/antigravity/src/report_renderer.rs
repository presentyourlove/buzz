//! 報告渲染器 - 每日進化雙層行動架構（極簡推播 + 風險分層 + 詳細報告）

use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::types::*;

pub struct ReportRenderer;

impl ReportRenderer {
    /// 生成頻道即時推播極簡版（30 秒速讀版，徹底去除流水帳與死板模組廢話）
    pub fn render_brief(report: &EvolutionReport) -> String {
        let mut brief = String::new();
        let date_str = report.date.format("%Y-%m-%d").to_string();

        brief.push_str(&format!("🐝 **【每日進化行動簡報】** ({})\n\n", date_str));

        // 1. 🎯 一句話核心結論
        let total_high_risk = report.summary.high_risk_actions.len();
        let total_auto = report.summary.autonomous_actions.len();
        let status_emoji = if report.success { "✅" } else { "⚠️" };

        brief.push_str(&format!(
            "🎯 **核心結論**：{} 全管線執行完成（耗時 {:.1}s）。今日識別出 **{} 項後台高危架構項** 與 **{} 項特工自主優化項**。\n\n",
            status_emoji, report.duration_seconds, total_high_risk, total_auto
        ));

        // 2. 🛡️ 【高風險 / 建議後台架構操作清單】（主理人 / Antigravity 執行）
        brief.push_str("🛡️ **【高風險 / 後台架構操作項】（主理人 / Antigravity 執行）**\n");
        if report.summary.high_risk_actions.is_empty() {
            brief.push_str("  - 🟢 今日無高危架構待辦，底層引擎與環境運作平穩。\n");
        } else {
            for (idx, item) in report.summary.high_risk_actions.iter().enumerate() {
                brief.push_str(&format!(
                    "  {}. **{}** (`{}`)\n     - **原因**：{}\n     - **為何需後台做**：{}\n",
                    idx + 1, item.title, item.target, item.reason, item.why_backend_only
                ));
                if !item.suggested_steps.is_empty() {
                    brief.push_str(&format!("     - **建議操作步驟**：{}\n", item.suggested_steps.join(" ➔ ")));
                }
            }
        }
        brief.push('\n');

        // 3. ⚡ 【低風險 / Buzz 特工自主優化清單】（Buzz / Pollen / Honey2 執行）
        brief.push_str("⚡ **【低風險 / Buzz 特工自主優化項】（Buzz 自主開工）**\n");
        if report.summary.autonomous_actions.is_empty() {
            brief.push_str("  - 🟢 今日無待派發之輕量腳本或 Prompt 優化項目。\n");
        } else {
            for (idx, item) in report.summary.autonomous_actions.iter().enumerate() {
                brief.push_str(&format!(
                    "  {}. **{}** ➔ 指派 @{}\n     - **目標檔案**：`{}`\n     - **優化目標**：{}\n     - **驗收標準**：{}\n",
                    idx + 1, item.title, item.assignee, item.target_file, item.goal, item.acceptance_criteria
                ));
            }
        }
        brief.push('\n');

        // 4. 📊 關鍵指標看板
        brief.push_str("📊 **關鍵指標快照**：\n");
        brief.push_str(&format!(
            "| 階段達成率 | 新增共識 | RCA發現 | 代碼健康警示 | 任務同步 | 自癒動作 |\n| :---: | :---: | :---: | :---: | :---: | :---: |\n| {}/{} | {} | {} | {} | {} | {} |\n",
            report.stages_completed,
            report.stages_total,
            report.summary.consensus_items,
            report.summary.rca_findings,
            report.summary.health_issues,
            report.summary.tasks_synced,
            report.summary.cicd_healed
        ));

        brief
    }

    /// 生成落盤詳細 Markdown 報告
    pub fn render_markdown(report: &EvolutionReport) -> String {
        let mut md = String::new();
        let date_str = report.date.format("%Y-%m-%d").to_string();

        md.push_str(&format!("# 每日系統進化與架構診斷詳細報告 - {}\n\n", date_str));
        md.push_str(&format!("> **產生時間**：{} | **版本**：v{} | **執行耗時**：{:.2} 秒\n\n", 
            report.metadata.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            report.metadata.version,
            report.duration_seconds
        ));

        // 執行摘要
        md.push_str("## 一、 執行摘要與風險分層\n\n");
        md.push_str(&Self::render_brief(report));
        md.push_str("\n---\n\n");

        // 階段結果明細
        md.push_str("## 二、 7 大管線執行細節\n\n");
        for stage in &report.stage_results {
            let status = if stage.success { "✅ 成功" } else { "❌ 失敗" };
            md.push_str(&format!("### {} ({}) - {}\n", stage.name, stage.stage.code(), status));
            md.push_str(&format!("- **耗時**：{:.3} 秒\n", stage.duration_seconds));
            if let Some(err) = &stage.error {
                md.push_str(&format!("- **異常訊息**：`{}`\n", err));
            }
            if !stage.metrics.is_empty() {
                md.push_str("- **指標**：");
                let metric_strs: Vec<String> = stage.metrics.iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                md.push_str(&metric_strs.join(", "));
                md.push('\n');
            }
            md.push('\n');
        }

        md
    }

    /// 輸出落盤檔案（支援專案集中路徑）
    pub fn write_outputs(report: &EvolutionReport, reports_dir: &Path, outbox_dir: &Path) -> Result<()> {
        fs::create_dir_all(reports_dir)?;
        fs::create_dir_all(outbox_dir)?;

        let date_suffix = report.date.format("%Y%m%d").to_string();

        // 1. 詳細 Markdown 報告
        let md_content = Self::render_markdown(report);
        let md_path = reports_dir.join(format!("DAILY_EVOLUTION_REPORT_{}.md", date_suffix));
        fs::write(&md_path, md_content)?;

        // 2. 頻道速讀極簡版
        let brief_content = Self::render_brief(report);
        let brief_path = reports_dir.join(format!("DAILY_EVOLUTION_BRIEF_{}.md", date_suffix));
        fs::write(&brief_path, brief_content)?;

        // 3. 完整 JSON 數據
        let json_content = serde_json::to_string_pretty(report)?;
        let json_path = outbox_dir.join(format!("DAILY_EVOLUTION_REPORT_{}.json", date_suffix));
        fs::write(&json_path, json_content)?;

        Ok(())
    }
}
