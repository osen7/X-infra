#!/bin/bash
set -e

# 构建 eBPF 程序（内核态）
echo "🔨 构建 eBPF 内核程序..."
cd xctl-probe-ebpf-ebpf
cargo +nightly build --release --target bpfel-unknown-none
cd ..

# 构建用户态程序
echo "🔨 构建用户态程序..."
cd xctl-probe-ebpf
cargo build --release
cd ..

echo "✅ 构建完成！"
echo "📦 可执行文件位置: xctl-probe-ebpf/target/release/xctl-probe-ebpf"
