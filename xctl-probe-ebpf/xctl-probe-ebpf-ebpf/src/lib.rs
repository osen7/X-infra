#![no_std]

/// Socket 四元组（用于映射到 PID）
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketTuple {
    pub src_ip: u32,      // 源 IP（IPv4，网络字节序）
    pub dst_ip: u32,      // 目的 IP（IPv4，网络字节序）
    pub src_port: u16,    // 源端口（网络字节序）
    pub dst_port: u16,    // 目的端口（网络字节序）
}

/// 网络事件结构体（内核态和用户态共享）
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetworkEvent {
    pub pid: u32,
    pub event_type: u8, // 1 = transport.drop
    pub retransmit_count: u32,
    pub timestamp: u64, // 纳秒级时间戳
    pub socket_tuple: SocketTuple, // Socket 四元组（用于调试和验证）
}
