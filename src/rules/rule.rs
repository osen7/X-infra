use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 规则数据结构
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub name: String,
    pub scene: String,
    pub priority: u32,
    pub conditions: Vec<Condition>,
    pub root_cause_pattern: RootCausePattern,
    pub solution_steps: Vec<SolutionStep>,
    pub related_evidences: Vec<String>,
    pub applicability: Applicability,
}

/// 规则条件
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Condition {
    /// 事件条件
    #[serde(rename = "event")]
    Event {
        event_type: String,
        entity_id_pattern: Option<String>,
        value_pattern: Option<String>,
        value_threshold: Option<f64>,
    },
    /// 图边条件
    #[serde(rename = "graph")]
    Graph {
        edge_type: String,
        from_pattern: Option<String>,
        to_pattern: Option<String>,
    },
}

/// 根因模式
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RootCausePattern {
    pub primary: String,
    pub secondary: Option<Vec<String>>,
}

/// 解决步骤
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SolutionStep {
    pub step: u32,
    pub action: String,
    pub command: Option<String>,
    #[serde(default)]
    pub manual: bool,
}

/// 适用条件
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Applicability {
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,
    pub required_events: Option<Vec<String>>,
}

fn default_min_confidence() -> f64 {
    0.8
}
