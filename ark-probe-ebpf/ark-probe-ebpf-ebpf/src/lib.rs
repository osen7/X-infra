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

/// 存储事件结构体（内核态和用户态共享）
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StorageEvent {
    pub pid: u32,
    pub event_type: u8, // 1 = io_latency, 2 = io_size
    pub io_latency_ns: u64, // I/O 延迟（纳秒）
    pub io_size: u64, // I/O 大小（字节）
    pub timestamp: u64, // 纳秒级时间戳
}

/// RDMA 事件结构体（内核态和用户态共享）
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RdmaEvent {
    pub pid: u32,
    pub event_type: u8, // 1 = rdma_congestion, 2 = pfc_storm, 3 = rdma_latency
    pub congestion_level: u32, // 拥塞级别
    pub pfc_count: u32, // PFC 帧计数
    pub latency_ns: u64, // RDMA 操作延迟（纳秒）
    pub timestamp: u64, // 纳秒级时间戳
}
