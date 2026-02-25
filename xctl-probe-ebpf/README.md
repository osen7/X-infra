# xctl eBPF 网络探针

这是 xctl 的 eBPF 网络探针，使用 Rust Aya 框架实现，直接从 Linux 内核态捕获 TCP 重传和丢包事件。

## 🎯 功能特性

- **零侵入**：不需要修改任何业务代码（PyTorch/MindSpore）
- **内核级监控**：Hook `tcp_retransmit_skb` 内核函数
- **实时事件流**：通过 PerfEventArray 实时输出 JSONL 格式事件
- **高性能**：eBPF 在内核态执行，开销极低

## 📋 前置要求

### 1. 安装 Rust 工具链

```bash
# 安装 nightly 工具链（用于编译 eBPF 内核代码）
rustup install nightly
rustup component add rust-src --toolchain nightly

# 安装 bpf-linker
cargo install bpf-linker
```

### 2. 安装系统依赖（Ubuntu/Debian）

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    clang \
    llvm \
    libelf-dev \
    linux-headers-$(uname -r)
```

### 3. 安装 Aya 依赖

```bash
cargo install cargo-generate
```

## 🔨 构建

```bash
# 方式 1：使用构建脚本
chmod +x build.sh
./build.sh

# 方式 2：手动构建
cd xctl-probe-ebpf-ebpf
cargo +nightly build --release --target bpfel-unknown-none
cd ../xctl-probe-ebpf
cargo build --release
```

## 🚀 运行

### 作为独立程序运行

```bash
# 输出 JSONL 格式（默认）
sudo ./target/release/xctl-probe-ebpf

# 输出调试格式
sudo ./target/release/xctl-probe-ebpf --format debug
```

### 集成到 xctl

```bash
# xctl 会自动通过 SubprocessProbe 启动此探针
xctl run --probe ./target/release/xctl-probe-ebpf
```

## 📊 输出格式

### JSONL 格式（默认）

```json
{"ts":1710000000000,"event_type":"transport.drop","entity_id":"network-pid-1024","pid":1024,"value":"1"}
{"ts":1710000001000,"event_type":"transport.drop","entity_id":"network-pid-1024","pid":1024,"value":"2"}
```

### 事件字段说明

- `ts`: 时间戳（毫秒）
- `event_type`: 事件类型（固定为 `transport.drop`）
- `entity_id`: 实体 ID（格式：`network-pid-<PID>`）
- `pid`: 触发重传的进程 PID
- `value`: 重传次数

## 🔧 工作原理

### 内核态（eBPF 程序）

1. **Hook 点**：`tcp_retransmit_skb` 内核函数
2. **触发时机**：当内核检测到 TCP 重传时
3. **数据采集**：
   - 当前进程 PID
   - 重传计数
   - 时间戳
4. **数据输出**：通过 `PerfEventArray` 发送到用户态

### 用户态（Rust 程序）

1. **加载 eBPF 程序**：将编译好的字节码加载到内核
2. **附加 kprobe**：将程序附加到 `tcp_retransmit_skb`
3. **监听事件**：异步读取 `PerfEventArray` 中的事件
4. **格式化输出**：将事件转换为 JSONL 格式，输出到 stdout

## 🐛 故障排除

### 权限问题

eBPF 程序需要 root 权限才能加载到内核：

```bash
sudo ./target/release/xctl-probe-ebpf
```

### 内核版本要求

- Linux 内核 >= 5.8（推荐）
- 支持 eBPF 和 kprobe

检查内核版本：

```bash
uname -r
```

### 编译错误

如果遇到编译错误，确保：

1. 已安装所有依赖
2. 使用正确的 Rust 工具链版本
3. 内核头文件已安装

## 📚 相关文档

- [Aya 框架文档](https://aya-rs.dev/)
- [eBPF 官方文档](https://ebpf.io/)
- [xctl 主项目](../README.md)

## 🔒 安全考虑

- eBPF 程序在内核态运行，必须经过严格验证
- 使用 `cargo build --release` 确保代码优化和安全性
- 生产环境建议启用 eBPF 验证器（内核默认启用）

## 🎯 未来扩展

- [ ] 支持 RDMA 网络监控
- [ ] 支持网络延迟统计
- [ ] 支持多网卡监控
- [ ] 支持网络拥塞检测（PFC Storm）
