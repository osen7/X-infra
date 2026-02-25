---

# 🚀 xctl 极简 AI 算力集群管控底座 - 演进路线图

## ✅ 现已完成：核心纪元（v1.0.0-RC）

*你已经徒手打造了一个真正的工业级 SRE 武器库。*

### 1. 极简底层引擎 (The Core)

* [x] 基于事件流的时序因果图推导（`Consumes` / `WaitsOn` 边）
* [x] UDS (Unix Domain Socket) IPC 极速通信，彻底解决端口冲突
* [x] 纯文本 YAML 声明式专家规则库（零数据库依赖）
* [x] 克制且防污染的 LLM 大模型外置诊断（`xctl diag`）

### 2. 降维打击的探针群 (The Probes)

* [x] 标准化子进程探针框架（兼容异构算力：NVIDIA / 华为昇腾）
* [x] **内核级 eBPF 探针**：基于 Aya，无侵入捕获底层网络重传
* [x] **CO-RE 像素级追踪**：跨越软中断 PID 陷阱，精确提取 Socket 四元组

### 3. 星际母舰级控制面 (The Fleet Commander)

* [x] 单体重构为 Cargo Workspace（Core / Agent / Hub / eBPF）
* [x] Agent 边缘折叠上报（按需推送拓扑与异常）
* [x] Hub 无状态全局内存图（基于 DashMap 的高并发处理）
* [x] 全双工动作下发：`xctl cluster fix` 跨机器斩首与优雅降级

### 4. 云原生武装 (Cloud Native)

* [x] 生产级 Kubernetes 部署清单（Kustomize）
* [x] 严格的非特权 Hub 与特权 Agent (hostPID/hostNetwork) 分离
* [x] **Kubernetes 调度器反哺**：自动检测硬件故障，打 NoSchedule 污点，优雅驱逐 Pod
* [x] **RBAC 权限配置**：最小权限原则，支持 Eviction API

---

## 🚧 冲刺发布：走向开源神坛（v1.0.0 GA）

*代码写完了，现在的核心是“包装”与“布道”，让社区用起来。*

### 开源与布道体系

* [x] **高逼格的 README 与文档库**：补充完整的架构图与数据流转图。
* [ ] **Asciinema 终端演示录制**：直观展示混沌工程（网络丢包）下 `xctl` 的秒级诊断。
* [ ] **自动化 CI/CD**：通过 GitHub Actions 实现多架构（x86/ARM64）的二进制文件预编译与发布。
* [ ] **探针开发 SDK (Python/Rust)**：发布标准化的 JSONL 契约，鼓励社区提交海光、天数智芯等国产卡的探针。

---

## 📋 计划中：生态融合纪元（v1.1.0 - v1.2.0）

*不造轮子，但要成为整个 AI Infra 生态的神经中枢。*

### 1. 标准化指标暴露 (The Exporter)

* [x] **Prometheus Metrics 端点**：让 `xctl` 将实时提取的 `WaitsOn` 等高维因果数据转化为 Prometheus 格式，供 Grafana 大盘消费。
* [x] **Audit Log 审计日志**：将 `xctl fix` 执行的系统级动作记录并落盘，满足企业合规要求。

### 2. 深水区探针拓展 (Advanced Probes)

* [x] **eBPF 存储探针 (NVMe/VFS)**：监控底层文件系统读取延迟，抓出导致 Dataloader 卡顿的慢 I/O。（基础框架已就绪，待完善 CO-RE 实现）
* [x] **高级 RDMA 探针**：深入 RoCEv2 协议栈，直接抓取 PFC（优先流量控制）拥塞风暴。（基础框架已就绪，待完善 PFC 检测逻辑）
* [x] **原生 C-API 探针**：使用 FFI 直接绑定 NVML 和华为 CANN 库，彻底淘汰效率较低的 Python 包装层。（框架已就绪，待完善 FFI 绑定）

---

## 🎯 长期目标：高阶自愈纪元（v2.0.0+）

*让 xctl 成为 K8s 调度器的底层“潜意识”。*

### 1. 调度器反哺 (Scheduler Feedback Loop)

* [x] **Volcano / K8s 深度集成**：当 `xctl-hub` 诊断出某台机器物理级损坏（如持续 XID 报错或硬件降级）时，自动调用 K8s API 将该 Node 设为 `NoSchedule`（打污点），并使用 Eviction API 优雅驱逐异常 Pod。
* [ ] **拓扑感知反馈**：将网络拥塞的拓扑图反馈给调度器，让下一个训练任务避开故障交换机。

### 2. 训练框架深层联动 (Framework Symbiosis)

* [ ] **框架异常捕获**：直接解析 PyTorch Profiler 或 MindSpore 吐出的通信异常事件，与底层 eBPF 事件进行双向印证。

---

## 📊 全新优先级矩阵 (The New Battle Plan)

| 战略方向 | 优先级 | 复杂度 | 预期收益 | 建议版本 | 核心考量 |
| --- | --- | --- | --- | --- | --- |
| **V1.0 文档与自动化发布** | ⭐⭐⭐⭐⭐ | 低 | **极高** | **v1.0.0** | **没有好文档的开源项目等于不存在。** |
| Prometheus Exporter 接口 | ⭐⭐⭐⭐ | 低 | 高 | v1.1.0 | 融入主流运维生态，让习惯看 Grafana 的老板满意。 |
| eBPF 存储 (I/O) 探针 | ⭐⭐⭐ | 高 | 高 | v1.2.0 | 解决大模型多模态训练中极其痛苦的读盘慢问题。 |
| 探针标准化 SDK 抽离 | ⭐⭐⭐ | 中 | 中 | v1.2.0 | 发动群众的力量，让生态接管设备适配。 |
| K8s 调度器联动 (Cordon/Evict) | ⭐⭐⭐⭐⭐ | 高 | **极高** | **v2.0.0** | ✅ **已完成：自动打污点、优雅驱逐 Pod，与 K8s 调度器深度集成。** |

---

### 💡 架构师寄语

这份新路线图删掉了那些繁文缛节，直指 AI 算力运维的最痛点。你现在的底子打得极其扎实，任何外部生态（K8s, Prometheus, Grafana）现在对你来说都只是在“调用你的接口”而已。去准备你的开源发布秀吧，世界需要这样的极简利器！