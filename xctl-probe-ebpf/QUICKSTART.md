# eBPF 网络探针快速开始

## 🚀 5 分钟快速部署

### 步骤 1：安装依赖

```bash
# Rust nightly 工具链
rustup install nightly
rustup component add rust-src --toolchain nightly

# bpf-linker
cargo install bpf-linker

# 系统依赖（Ubuntu/Debian）
sudo apt-get install -y build-essential clang llvm libelf-dev linux-headers-$(uname -r)
```

### 步骤 2：构建

```bash
cd xctl-probe-ebpf
./build.sh
```

### 步骤 3：运行

```bash
# 测试运行（需要 root）
sudo ./xctl-probe-ebpf/target/release/xctl-probe-ebpf

# 集成到 xctl
xctl run --probe ./xctl-probe-ebpf/target/release/xctl-probe-ebpf
```

## 📊 验证

运行后，你应该看到类似这样的输出：

```json
{"ts":1710000000000,"event_type":"transport.drop","entity_id":"network-pid-1024","pid":1024,"value":"1"}
{"ts":1710000001000,"event_type":"transport.drop","entity_id":"network-pid-2048","pid":2048,"value":"1"}
```

## 🎯 下一步

- 查看 [完整文档](./README.md)
- 阅读 [实现指南](../docs/EBPF_NETWORK_PROBE.md)
- 集成到 xctl 主项目
