# ky 平台痛点集成指南

本文档说明如何将 ky 平台（华为昇腾/GPU 生产调度环境）的核心痛点转化为 xctl 的杀手级特性。

## 已实现的痛点解决方案

### 1. NPU 亚健康故障检测 ✅

**ky 痛点**：SOC 过温和 HCCS 降级等亚健康状态，不会让进程立刻死掉，但会严重拖慢训练。

**xctl 实现**：
- 规则文件：`rules/npu-subhealth.yaml`
- 分析器：`src/scene/npu_subhealth.rs`
- 检测指标：
  - 温度 > 85°C（SOC 过温）
  - HCCS 链路状态 = "degraded"
  - 频率降频（低于最大频率的 90%）

**使用示例**：
```bash
xctl why <pid>  # 自动识别亚健康场景
xctl diag <pid> --rules-dir rules/  # 使用规则引擎匹配
```

### 2. 智能工作负载卡死检测 ✅

**ky 痛点**：死板的超时杀死（默认 5 分钟），容易误杀正在做 CPU 数据预处理的正常任务。

**xctl 实现**：
- 规则文件：`rules/workload-stalled.yaml`
- 分析器：`src/scene/workload_stalled.rs`
- 智能判断逻辑：
  - 进程状态 = running
  - 所有消耗的资源利用率 < 1%
  - **且**没有等待网络或存储 IO
  - → 才是真正的死锁/卡死

**优势**：传统平台无法做到这种精准度，xctl 通过图引擎的 WaitsOn 边可以准确区分"卡死"和"正常 IO 等待"。

### 3. RoCE 与 HCCS 网络区分 ✅

**ky 痛点**：Ascend910C 多机通信，同超节点用 HCCS，跨节点用 RoCE，需要区分处理。

**xctl 实现**：
- `rules/roce-congestion.yaml`：跨节点 RoCE 网络拥塞
- `rules/hccs-error.yaml`：同超节点 HCCS 链路错误
- 通过 `entity_id_pattern` 区分网络类型

**使用场景**：
```yaml
# RoCE 拥塞
- type: "graph"
  edge_type: "waits_on"
  to_pattern: "roce-*|eth*"

# HCCS 错误
- type: "graph"
  edge_type: "blocked_by"
  to_pattern: "hccs-*"
```

## 架构改进

### 1. 数值解析隐患修复 ✅

**问题**：`parse::<f64>()` 会将 "D" (Disk Sleep) 误解析为 0.0。

**解决方案**：
- 新增 `ValueType` 枚举（Numeric/String/Auto）
- 新增 `ComparisonOp` 枚举（Gt/Lt/Eq/Gte/Lte/Ne/Contains）
- 新增 `MetricCondition` 结构，支持类型安全的比较

**示例**：
```yaml
metrics:
  - key: "temperature"
    op: "gt"
    target: "85"
    value_type: "numeric"  # 明确指定数值类型
  - key: "state"
    op: "eq"
    target: "D"
    value_type: "string"   # 字符串比较，不会误解析
```

### 2. 条件逻辑增强 ✅

**新增功能**：
- `any` 条件：OR 逻辑（任意一个满足即可）
- `all` 条件：AND 逻辑（所有条件都必须满足）
- `metric` 条件：直接匹配节点 metadata

**示例**：
```yaml
conditions:
  any:  # OR 逻辑
    - type: "metric"
      metrics:
        - key: "temperature"
          op: "gt"
          target: "85"
    - type: "metric"
      metrics:
        - key: "hccs_lane_status"
          op: "eq"
          target: "degraded"
```

### 3. 推荐操作增强 ✅

**新增字段**：
- `recommended_actions`：为未来的 `xctl fix` 命令铺路
- `severity`：严重程度（Critical/Warning/Info）

**示例**：
```rust
recommended_actions: vec![
    "尝试触发框架层的 Checkpoint Dump 信号 (SIGUSR1)".to_string(),
    "隔离该节点，执行 xctl zap 清理僵尸进程".to_string(),
    "修改批量大小 (Batch Size) 后重提任务".to_string(),
],
```

## 未来扩展方向

### 1. Checkpoint 恢复支持

在 `recommended_actions` 中已经预留了 Checkpoint 相关操作，未来可以实现：
- 自动触发 Checkpoint Dump
- 从 Checkpoint 恢复训练
- Checkpoint 完整性验证

### 2. 多网络类型支持

当前已区分 RoCE 和 HCCS，未来可以扩展：
- InfiniBand 支持
- 网络拓扑感知（同节点/跨节点）
- 网络性能基线对比

### 3. 亚健康趋势分析

当前只检测瞬时状态，未来可以：
- 记录亚健康历史趋势
- 预测硬件故障
- 自动触发预防性维护

## 与 ky 平台的对比

| 特性 | ky 平台 | xctl |
|------|---------|------|
| 亚健康检测 | 无 | ✅ NPU 亚健康分析器 |
| 智能卡死判断 | 固定超时 | ✅ 基于图引擎的精准判断 |
| 网络类型区分 | 无 | ✅ RoCE/HCCS 区分 |
| 规则扩展性 | 硬编码 | ✅ YAML 声明式规则 |
| 根因分析 | 基础 | ✅ 因果图深度推导 |

## 总结

xctl 通过以下设计实现了对 ky 平台痛点的降维打击：

1. **极简规则引擎**：YAML 声明式，零数据库依赖
2. **智能场景分析**：基于因果图的深度推导
3. **类型安全匹配**：避免数值解析陷阱
4. **可扩展架构**：Trait 设计，易于添加新场景

这些特性使得 xctl 不仅能解决 ky 平台的痛点，还能持续演进，适应新的故障模式。
