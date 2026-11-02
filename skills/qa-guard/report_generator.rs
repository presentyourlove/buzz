//! Report Generator Module
//! 
//! Generates comprehensive validation reports for code quality assurance
//! with detailed metrics, statistics, and actionable recommendations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub report_id: String,
    pub timestamp: i64,
    pub project_path: String,
    pub overall_score: f32,
    pub total_issues: usize,
    pub critical_issues: usize,
    pub high_issues: usize,
    pub medium_issues: usize,
    pub low_issues: usize,
    pub info_items: usize,
    pub files_analyzed: usize,
    pub validation_types: HashMap<String, usize>,
    pub severity_distribution: HashMap<String, usize>,
    pub recommendations: Vec<Recommendation>,
    pub summary: String,
    pub chart_data: ChartData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub category: RecommendationCategory,
    pub message: String,
    pub priority: Priority,
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub action_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    Security,
    Performance,
    CodeQuality,
    Dependencies,
    Testing,
    Documentation,
    Configuration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
    Optional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub pie_chart: PieChartData,
    pub bar_chart: BarChartData,
    pub line_chart: LineChartData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieChartData {
    pub labels: Vec<String>,
    pub data: Vec<f32>,
    pub colors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarChartData {
    pub labels: Vec<String>,
    pub datasets: Vec<ChartDataset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataset {
    pub label: String,
    pub data: Vec<f32>,
    pub background_color: String,
    pub border_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineChartData {
    pub labels: Vec<String>,
    pub datasets: Vec<ChartDataset>,
}

pub struct ReportGenerator {
    config: ReportConfig,
    template_engine: TemplateEngine,
}

impl ReportGenerator {
    pub fn new() -> Self {
        let config = ReportConfig {
            include_charts: true,
            include_recommendations: true,
            export_formats: vec!["json".to_string(), "html".to_string(), "text".to_string()],
            report_title: "Code Quality Validation Report".to_string(),
            company_logo_path: None,
        };
        
        let template_engine = TemplateEngine::new();
        
        Self { config, template_engine }
    }
    
    /// Generate comprehensive validation report
    pub fn generate_report(&self, project_path: &str, results: &[ValidationResult]) -> Result<ValidationReport, String> {
        let report_id = format!("report_{}", uuid::Uuid::new_v4());
        let timestamp = chrono::Utc::now().timestamp();
        
        let mut total_issues = 0;
        let mut critical_issues = 0;
        let mut high_issues = 0;
        let mut medium_issues = 0;
        let mut low_issues = 0;
        let mut info_items = 0;
        let mut files_analyzed = 0;
        let mut validation_types: HashMap<String, usize> = HashMap::new();nosecondcontent;
        let mut severity_distribution: HashMap<String, usize> = HashMap::new();nosecondcontent;
        
        for result in results {
            files_analyzed += 1;
            
            for error in &result.errors {
                total_issues += 1;
                match error.severity {
                    Severity::Critical => critical_issues += 1,
                    Severity::High => high_issues += 1,
                    Severity::Medium => medium_issues += 1,
                    Severity::Low => low_issues += 1,
                    Severity::Info => info_items += 1,
                }
                
                let severity_str = match error.severity {
                    Severity::Critical => "Critical",
                    Severity::High => "High",
                    Severity::Medium => "Medium",
                    Severity::Low => "Low",
                    Severity::Info => "Info",
                };
                *severity_distribution.entry(severity_str.to_string()).or_insert(0) += 1;
            }
            
            let validation_type_str = match result.validation_type {
                ValidationType::SyntaxCheck => "Syntax".to_string(),
                ValidationType::SecurityScan => "Security".to_string(),
                ValidationType::PerformanceAnalysis => "Performance".to_string(),
                ValidationType::StyleCheck => "Style".to_string(),
                ValidationType::DependencyCheck => "Dependencies".to_string(),
                ValidationType::TestCoverage => "Test Coverage".to_string(),
            };
            *validation_types.entry(validation_type_str).or_insert(0) += 1;
        }
        
        let overall_score = self.calculate_overall_score(results);
        let recommendations = self.generate_recommendations(results);
        let summary = self.generate_summary(&results);
        let chart_data = self.generate_chart_data(&severity_distribution, &validation_types);
        
        Ok(ValidationReport {
            report_id,
            timestamp,
            project_path: project_path.to_string(),
            overall_score,
            total_issues,
            critical_issues,
            high_issues,
            medium_issues,
            low_issues,
            info_items,
            files_analyzed,
            validation_types,
            severity_distribution,
            recommendations,
            summary,
            chart_data,
        })
    }
    
    /// Export report to different formats
    pub fn export_report(&self, report: &ValidationReport, output_path: &str, format: &str) -> Result<(), String> {
        match format.to_lowercase().as_str() {
            "json" => self.export_json(report, output_path),
            "html" => self.export_html(report, output_path),
            "text" => self.export_text(report, output_path),
            _ => Err(format!("Unsupported export format: {}", format)),
        }
    }
    
    /// Generate recommendations based on validation results
    fn generate_recommendations(&self, results: &[ValidationResult]) -> Vec<Recommendation> {
        let mut recommendations: Vec<Recommendation> = Vec::new();nosecondcontent;
        
        for result in results {
            for error in &result.errors {
                let category = match error.validation_type {
                    ValidationType::SecurityScan => RecommendationCategory::Security,
                    ValidationType::PerformanceAnalysis => RecommendationCategory::Performance,
                    ValidationType::SyntaxCheck => RecommendationCategory::CodeQuality,
                    ValidationType::DependencyCheck => RecommendationCategory::Dependencies,
                    ValidationType::TestCoverage => RecommendationCategory::Testing,
                    ValidationType::StyleCheck => RecommendationCategory::CodeQuality,
                };
                
                let priority = match error.severity {
                    Severity::Critical => Priority::Critical,
                    Severity::High => Priority::High,
                    Severity::Medium => Priority::Medium,
                    Severity::Low => Priority::Low,
                    Severity::Info => Priority::Optional,
                };
                
                let mut action_items = Vec::new();
                match error.validation_type {
                    ValidationType::SecurityScan => {
                        action_items.push("Review and fix security vulnerabilities immediately".to_string());
                        action_ite
