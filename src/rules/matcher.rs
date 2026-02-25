use crate::event::Event;
use crate::graph::{EdgeType, StateGraph};
use crate::rules::rule::Condition;

/// 规则匹配器
pub struct RuleMatcher;

impl RuleMatcher {
    /// 检查规则条件是否满足（异步版本）
    pub async fn match_condition(
        condition: &Condition,
        events: &[Event],
        graph: &StateGraph,
    ) -> bool {
        match condition {
            Condition::Event {
                event_type,
                entity_id_pattern,
                value_pattern,
                value_threshold,
            } => {
                events.iter().any(|event| {
                    // 匹配事件类型
                    if event.event_type.to_string() != *event_type {
                        return false;
                    }

                    // 匹配实体 ID 模式
                    if let Some(pattern) = entity_id_pattern {
                        if !matches_pattern(&event.entity_id, pattern) {
                            return false;
                        }
                    }

                    // 匹配值模式
                    if let Some(pattern) = value_pattern {
                        if !event.value.contains(pattern) {
                            return false;
                        }
                    }

                    // 匹配值阈值
                    if let Some(threshold) = value_threshold {
                        if let Ok(value) = event.value.parse::<f64>() {
                            if value < *threshold {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }

                    true
                })
            }
            Condition::Graph {
                edge_type,
                from_pattern,
                to_pattern,
            } => {
                // 获取图中的所有边（异步）
                let edges = graph.get_all_edges_async().await;
                
                edges.iter().any(|edge| {
                    // 匹配边类型
                    let edge_type_str = match edge.edge_type {
                        EdgeType::Consumes => "consumes",
                        EdgeType::WaitsOn => "waits_on",
                        EdgeType::BlockedBy => "blocked_by",
                    };
                    
                    if edge_type_str != edge_type.as_str() {
                        return false;
                    }

                    // 匹配源节点模式
                    if let Some(pattern) = from_pattern {
                        if !matches_pattern(&edge.from, pattern) {
                            return false;
                        }
                    }

                    // 匹配目标节点模式
                    if let Some(pattern) = to_pattern {
                        if !matches_pattern(&edge.to, pattern) {
                            return false;
                        }
                    }

                    true
                })
            }
        }
    }

    /// 检查所有条件是否满足（异步版本）
    pub async fn match_all_conditions(
        conditions: &[Condition],
        events: &[Event],
        graph: &StateGraph,
    ) -> bool {
        for condition in conditions {
            if !Self::match_condition(condition, events, graph).await {
                return false;
            }
        }
        true
    }
}

/// 简单的通配符模式匹配
/// 支持 * 通配符（如 "gpu-*"）
fn matches_pattern(text: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            // 简单的前缀和后缀匹配
            text.starts_with(parts[0]) && text.ends_with(parts[1])
        } else if parts.len() == 1 {
            // 只有前缀或后缀
            text.starts_with(parts[0]) || text.ends_with(parts[0])
        } else {
            // 多个 * 暂不支持，使用精确匹配
            text == pattern
        }
    } else {
        text == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("gpu-0", "gpu-*"));
        assert!(matches_pattern("gpu-1", "gpu-*"));
        assert!(!matches_pattern("cpu-0", "gpu-*"));
        assert!(matches_pattern("mlx5_0", "mlx5_*"));
    }
}
