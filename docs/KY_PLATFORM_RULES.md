# ky 平台专属规则与场景分析器

本文档说明已为 ky 平台（国产算力）实现的专属故障检测规则和分析器。

## 已实现的场景分析器

### 1. NPU 亚健康检测 (`NpuSubhealthAnalyzer`)
- **场景类型**: `NpuSubhealth`
- **检测内容**:
  - SOC 过温（> 85°C）
  - HCCS 链路降级
  - 频率降频（低于最大频率的 90%）
- **规则文件**: `rules/npu-subhealth.yaml`
- **严重程度**: Warning

### 2. 工作负载卡死检测 (`WorkloadStalledAnalyzer`)
- **场景类型**: `WorkloadStalled`
- **检测逻辑**:
  - 进程状态为 `running`
  - 所有 GPU/NPU 资源利用率 < 1%
  - 没有等待网络或存储 IO
- **规则文件**: `rules/workload-stalled.yaml`
- **严重程度**: Warning
- **优势**: 智能判断，避免误杀正常的数据预处理阶段

### 3. GPU 利用率低检测 (`GpuUtilLowAnalyzer`)
- **场景类型**: `GpuUtilLow`
- **检测内容**: GPU/NPU 利用率 < 10%
- **规则文件**: `rules/gpu-util-low.yaml`
- **严重程度**: Warning

### 4. 存储 IO 错误检测 (`StorageIoErrorAnalyzer`)
- **场景类型**: `StorageIoError`
- **检测内容**: 存储设备 IO 错误
- **规则文件**: `rules/storage-io-error.yaml`
- **严重程度**: Critical

### 5. 存储慢速检测 (`StorageSlowAnalyzer`)
- **场景类型**: `StorageSlow`
- **检测内容**: 存储 IOPS < 100 或延迟 > 100ms
- **规则文件**: `rules/storage-slow.yaml`
- **严重程度**: Warning

### 6. Checkpoint 超时检测 (`CheckpointTimeoutAnalyzer`)
- **场景类型**: `ProcessBlocked` (复用)
- **检测内容**: Checkpoint 保存/加载超时
- **规则文件**: `rules/checkpoint-timeout.yaml`
- **严重程度**: Warning

## 已实现的规则文件

### NPU 相关规则
1. **npu-subhealth.yaml** - NPU 亚健康降级场景
2. **npu-ecc-error.yaml** - NPU ECC 内存错误场景

### 网络相关规则
3. **roce-congestion.yaml** - RoCE 拥塞场景
4. **hccs-error.yaml** - HCCS 错误场景
5. **network-stall.yaml** - 网络阻塞场景

### 存储相关规则
6. **storage-io-error.yaml** - 存储 IO 错误场景
7. **storage-slow.yaml** - 存储慢速场景
8. **dataloader-io-error.yaml** - 数据加载 IO 错误场景
9. **checkpoint-timeout.yaml** - Checkpoint 超时场景

### GPU/工作负载相关规则
10. **gpu-oom.yaml** - GPU OOM 场景
11. **gpu-util-low.yaml** - GPU 利用率低场景
12. **workload-stalled.yaml** - 工作负载卡死场景
13. **process-crash.yaml** - 进程崩溃场景

## 场景类型枚举

所有场景类型定义在 `src/scene/types.rs`:

```rust
pub enum SceneType {
    // GPU 相关
    GpuOom,
    GpuUtilLow,
    GpuError,
    
    // NPU 相关（ky 平台）
    NpuSubhealth,
    WorkloadStalled,
    
    // 网络相关
    NetworkStall,
    NetworkDrop,
    
    // 存储相关
    StorageIoError,
    StorageSlow,
    
    // 进程相关
    ProcessBlocked,
    ProcessCrash,
}
```

## 使用方式

### 1. 规则引擎自动匹配
规则引擎会在 `xctl diag <pid>` 时自动加载 `rules/` 目录下的所有 YAML 规则，按优先级匹配。

### 2. 场景分析器手动调用
```rust
use crate::scene::SceneIdentifier;

let identifier = SceneIdentifier::new();
let scene = identifier.identify_scene(&graph, pid).await;
if let Some(scene) = scene {
    let result = identifier.analyze_scene(scene, &graph, pid).await;
}
```

## 规则优先级

规则按 `priority` 字段排序，数值越大优先级越高：
- 100: NPU ECC 错误（最严重）
- 95: NPU 亚健康
- 90: 存储 IO 错误、GPU OOM
- 85: Checkpoint 超时、数据加载 IO 错误
- 80: GPU 利用率低
- 75: 存储慢速

## 未来扩展

可以继续添加的场景：
- NPU 固件版本不匹配
- 训练框架死锁检测
- 分布式训练同步超时
- 数据预处理瓶颈检测

## 相关文档

- [规则引擎文档](./RULES_ENGINE.md)
- [ky 平台集成文档](./KY_PLATFORM_INTEGRATION.md)
