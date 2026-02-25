use crate::event::Event;
use crate::graph::{EdgeType, NodeType, StateGraph};
use crate::rules::rule::{ComparisonOp, Condition, MetricCondition, ValueType};

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

                    // 匹配值阈值（改进：更安全的数值解析）
                    if let Some(threshold) = value_threshold {
                        match event.value.parse::<f64>() {
                            Ok(value) => {
                                if value < *threshold {
                                    return false;
                                }
                            }
                            Err(_) => {
                                // 如果无法解析为数值，且阈值存在，则不匹配
                                // 这避免了将 "D" (Disk Sleep) 误解析为 0.0
                                return false;
                            }
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
            Condition::Metric {
                node_type,
                entity_id_pattern,
                metrics,
            } => {
                // 获取所有节点
                let nodes = graph.get_nodes_async().await;
                
                nodes.values().any(|node| {
                    // 匹配节点类型
                    if let Some(ref nt) = node_type {
                        let node_type_str = match node.node_type {
                            NodeType::Process => "process",
                            NodeType::Resource => "resource",
                            NodeType::Error => "error",
                        };
                        if node_type_str != nt.as_str() {
                            return false;
                        }
                    }
                    
                    // 匹配实体 ID 模式
                    if let Some(ref pattern) = entity_id_pattern {
                        if !matches_pattern(&node.id, pattern) {
                            return false;
                        }
                    }
                    
                    // 匹配所有指标条件
                    metrics.iter().all(|metric| {
                        match_metric_condition(metric, &node.metadata)
                    })
                })
            }
            Condition::Any { conditions } => {
                // OR 逻辑：任意一个条件满足即可
                for condition in conditions {
                    if Self::match_condition(condition, events, graph).await {
                        return true;
                    }
                }
                false
            }
            Condition::All { conditions } => {
                // AND 逻辑：所有条件都必须满足
                for condition in conditions {
                    if !Self::match_condition(condition, events, graph).await {
                        return false;
                    }
                }
                true
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

/// 匹配指标条件（支持数值和字符串比较）
fn match_metric_condition(metric: &MetricCondition, metadata: &std::collections::HashMap<String, String>) -> bool {
    let actual_str = match metadata.get(&metric.key) {
        Some(v) => v,
        None => return false,
    };
    
    match metric.value_type {
        ValueType::Numeric => {
            // 数值比较
            let actual_val = match actual_str.parse::<f64>() {
                Ok(v) => v,
                Err(_) => return false, // 无法解析为数值，不匹配
            };
            
            let target_val = match metric.target.parse::<f64>() {
                Ok(v) => v,
                Err(_) => return false,
            };
            
            match metric.op {
                ComparisonOp::Gt => actual_val > target_val,
                ComparisonOp::Lt => actual_val < target_val,
                ComparisonOp::Eq => (actual_val - target_val).abs() < 0.001, // 浮点数比较
                ComparisonOp::Gte => actual_val >= target_val,
                ComparisonOp::Lte => actual_val <= target_val,
                ComparisonOp::Ne => (actual_val - target_val).abs() >= 0.001,
                ComparisonOp::Contains => actual_str.contains(&metric.target),
            }
        }
        ValueType::String => {
            // 字符串比较
            match metric.op {
                ComparisonOp::Eq => actual_str == metric.target,
                ComparisonOp::Ne => actual_str != metric.target,
                ComparisonOp::Contains => actual_str.contains(&metric.target),
                _ => false, // 其他操作符对字符串无效
            }
        }
        ValueType::Auto => {
            // 自动检测：先尝试数值，失败则用字符串
            if let (Ok(actual_val), Ok(target_val)) = (actual_str.parse::<f64>(), metric.target.parse::<f64>()) {
                // 数值比较
                match metric.op {
                    ComparisonOp::Gt => actual_val > target_val,
                    ComparisonOp::Lt => actual_val < target_val,
                    ComparisonOp::Eq => (actual_val - target_val).abs() < 0.001,
                    ComparisonOp::Gte => actual_val >= target_val,
                    ComparisonOp::Lte => actual_val <= target_val,
                    ComparisonOp::Ne => (actual_val - target_val).abs() >= 0.001,
                    ComparisonOp::Contains => actual_str.contains(&metric.target),
                }
            } else {
                // 字符串比较
                match metric.op {
                    ComparisonOp::Eq => actual_str == metric.target,
                    ComparisonOp::Ne => actual_str != metric.target,
                    ComparisonOp::Contains => actual_str.contains(&metric.target),
                    _ => false,
                }
            }
        }
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
