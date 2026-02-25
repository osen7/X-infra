# xctl 指标导出设计（Prometheus Exporter）

## 设计原则

**核心原则**：xctl 不造轮子，不成为 Prometheus 的替代品。只提供标准化导出接口。

## 架构设计

### Prometheus Exporter 模式

xctl 作为 Prometheus Exporter，暴露 `/metrics` 端点，让 Prometheus 主动拉取。

### 指标定义

xctl 只导出**事件和因果关系相关的指标**，不导出原始资源指标（这些由 Node Exporter、GPU Exporter 等提供）。

```rust
// src/metrics/mod.rs

use prometheus::{Encoder, TextEncoder, Registry, Gauge, Counter, Histogram};

pub struct XctlMetrics {
    // 事件计数
    events_total: Counter,
    events_by_type: CounterVec,
    
    // 因果关系指标
    waitson_edges: Gauge,           // 当前 WaitsOn 边数量
    blockedby_edges: Gauge,         // 当前 BlockedBy 边数量
    consumes_edges: Gauge,           // 当前 Consumes 边数量
    
    // 诊断指标
    diagnoses_total: Counter,
    diagnoses_by_scene: CounterVec,
    diagnosis_latency: Histogram,
    
    // 图状态指标
    active_processes: Gauge,
    active_resources: Gauge,
    error_nodes: Gauge,
}

impl XctlMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        
        Ok(Self {
            events_total: Counter::new("xctl_events_total", "Total events processed")?,
            events_by_type: CounterVec::new(
                Opts::new("xctl_events_by_type", "Events by type"),
                &["event_type"]
            )?,
            waitson_edges: Gauge::new("xctl_waitson_edges", "Current WaitsOn edges")?,
            blockedby_edges: Gauge::new("xctl_blockedby_edges", "Current BlockedBy edges")?,
            consumes_edges: Gauge::new("xctl_consumes_edges", "Current Consumes edges")?,
            diagnoses_total: Counter::new("xctl_diagnoses_total", "Total diagnoses")?,
            diagnoses_by_scene: CounterVec::new(
                Opts::new("xctl_diagnoses_by_scene", "Diagnoses by scene"),
                &["scene"]
            )?,
            diagnosis_latency: Histogram::with_opts(
                HistogramOpts::new("xctl_diagnosis_latency_seconds", "Diagnosis latency")
            )?,
            active_processes: Gauge::new("xctl_active_processes", "Active processes")?,
            active_resources: Gauge::new("xctl_active_resources", "Active resources")?,
            error_nodes: Gauge::new("xctl_error_nodes", "Error nodes")?,
        })
    }
    
    pub fn export_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}
```

### HTTP 端点

```rust
// 在 main.rs 的 run_daemon 中

// 启动 Prometheus Exporter（可选）
if let Some(metrics_port) = metrics_port {
    let metrics_handle = {
        let graph = Arc::clone(&graph);
        tokio::spawn(async move {
            let metrics = XctlMetrics::new().unwrap();
            let server = MetricsServer::new(metrics, metrics_port);
            server.serve().await;
        })
    };
}
```

### 使用方式

```bash
# 启动 daemon，启用指标导出
cargo run --release -- run --metrics-port 9091

# Prometheus 配置
# prometheus.yml
scrape_configs:
  - job_name: 'xctl'
    static_configs:
      - targets: ['localhost:9091']
```

### 导出的指标示例

```
# 事件计数
xctl_events_total 12345
xctl_events_by_type{event_type="compute.util"} 5000
xctl_events_by_type{event_type="transport.drop"} 100

# 因果关系
xctl_waitson_edges 15
xctl_blockedby_edges 3
xctl_consumes_edges 50

# 诊断指标
xctl_diagnoses_total 10
xctl_diagnoses_by_scene{scene="gpu_oom"} 5
xctl_diagnosis_latency_seconds{quantile="0.5"} 1.2

# 图状态
xctl_active_processes 20
xctl_active_resources 8
xctl_error_nodes 2
```

## 与 Grafana 集成

Grafana 从 Prometheus 拉取数据，生成报表和仪表盘。

xctl 只负责：
1. 导出标准化指标
2. 提供实时事件流（通过 IPC）

不负责：
1. 历史数据存储
2. 报表生成
3. 可视化

## 优势

1. **不造轮子**：利用 Prometheus 生态
2. **标准化**：遵循 Prometheus 指标格式
3. **轻量级**：Daemon 只增加一个 HTTP 端点
4. **可扩展**：与现有监控栈无缝集成
