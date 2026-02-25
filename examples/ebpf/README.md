# eBPF 网络探针

这是 xctl 的 eBPF 网络探针实现，用于监控 TCP 重传、网络延迟和丢包。

## 前置要求

1. **Linux 内核 5.8+**（支持 ring buffer）
2. **root 权限**或 `CAP_BPF` 能力
3. **libbpf 库**（通常通过 libbpf-rs 或 libbpf-sys 提供）
4. **clang** 和 **LLVM**（用于编译 eBPF 程序）

## 编译

### 编译 eBPF 程序

```bash
clang -O2 -target bpf -c network_probe.bpf.c -o network_probe.bpf.o
```

### 编译用户空间程序

如果使用 Rust：

```bash
cargo build --release --bin xctl-probe-ebpf
```

## 使用

### 作为独立探针运行

```bash
sudo ./target/release/xctl-probe-ebpf
```

### 集成到 xctl

```bash
xctl run --probe ./target/release/xctl-probe-ebpf
```

## 输出格式

探针输出 JSONL 格式的事件，每行一个 JSON 对象：

```json
{"ts": 1234567890, "type": "transport.drop", "entity_id": "eth0", "pid": 12345, "value": "1"}
{"ts": 1234567891, "type": "transport.bw", "entity_id": "eth0", "pid": 12345, "value": "1000000"}
```

## 事件类型

- `transport.drop`: 网络丢包
- `transport.bw`: 网络带宽
- `error.net`: 网络错误（重传等）

## 注意事项

1. eBPF 程序需要 root 权限运行
2. 某些内核版本可能不支持所有功能
3. 生产环境建议使用 BCC 或 libbpf-rs 等成熟框架

## 未来改进

- [ ] 支持 RDMA 监控
- [ ] 支持更细粒度的延迟统计
- [ ] 支持网络拥塞检测
- [ ] 支持多网卡监控
