//! 内核结构体绑定（由 aya-tool 生成）
//! 
//! 此文件应该通过运行 generate-bindings.sh 生成
//! 这里提供一个示例结构，实际使用时需要从 /sys/kernel/btf/vmlinux 生成

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

/// sock_common 结构体（socket 的公共部分）
/// 这是从内核 BTF 信息中提取的
#[repr(C)]
pub struct sock_common {
    // 注意：实际字段布局由内核版本决定
    // 这里只是示例，实际应该从 BTF 生成
    pub skc_family: u16,
    pub skc_state: u8,
    pub skc_reuse: u8,
    pub skc_bound_dev_if: u32,
    pub skc_hash: u32,
    pub skc_portpair: u32,
    pub skc_num: u16,              // 源端口
    pub skc_dport: u16,            // 目的端口（网络字节序）
    pub skc_addrpair: u64,
    pub skc_daddr: u32,            // 目的 IP（IPv4，网络字节序）
    pub skc_rcv_saddr: u32,        // 源 IP（IPv4，网络字节序）
    // ... 其他字段
}

/// sock 结构体（完整的 socket）
#[repr(C)]
pub struct sock {
    pub __sk_common: sock_common,
    // ... 其他字段
}
