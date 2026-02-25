#!/usr/bin/env python3
"""
Ark 网络探针：监控网络 I/O 阻塞和延迟
基于 /proc/net 和系统调用追踪

输出格式：每行一个 JSON 对象（JSONL）
"""

import json
import time
import sys
import os
import subprocess
import re
from collections import defaultdict
from typing import Dict, List, Tuple, Optional

# 全局状态
prev_stats = {}  # 上一次的网络统计
socket_pids = {}  # socket -> pid 映射（通过 /proc/net）

def get_network_interfaces():
    """获取所有网络接口"""
    try:
        with open('/proc/net/dev', 'r') as f:
            lines = f.readlines()
        interfaces = []
        for line in lines[2:]:  # 跳过表头
            parts = line.split(':')
            if len(parts) == 2:
                interface = parts[0].strip()
                if interface and interface != 'lo':  # 排除回环接口
                    interfaces.append(interface)
        return interfaces
    except FileNotFoundError:
        return []

def parse_net_dev_stats():
    """解析 /proc/net/dev 获取网络统计"""
    stats = {}
    try:
        with open('/proc/net/dev', 'r') as f:
            lines = f.readlines()
        for line in lines[2:]:
            parts = line.split(':')
            if len(parts) != 2:
                continue
            interface = parts[0].strip()
            data = parts[1].split()
            if len(data) >= 16:
                stats[interface] = {
                    'rx_bytes': int(data[0]),
                    'rx_packets': int(data[1]),
                    'rx_drops': int(data[3]),
                    'rx_errors': int(data[2]),
                    'tx_bytes': int(data[8]),
                    'tx_packets': int(data[9]),
                    'tx_drops': int(data[11]),
                    'tx_errors': int(data[10]),
                }
    except (FileNotFoundError, ValueError, IndexError):
        pass
    return stats

def get_socket_to_pid_mapping():
    """通过 /proc/net 获取 socket 到 PID 的映射"""
    mapping = {}
    
    # TCP sockets
    try:
        with open('/proc/net/tcp', 'r') as f:
            for line in f.readlines()[1:]:  # 跳过表头
                parts = line.split()
                if len(parts) >= 10:
                    local_addr = parts[1]
                    inode = int(parts[9])
                    # 通过 inode 查找 PID（需要遍历 /proc）
                    pid = find_pid_by_inode(inode, 'tcp')
                    if pid:
                        mapping[f"tcp:{local_addr}"] = pid
    except (FileNotFoundError, ValueError):
        pass
    
    # UDP sockets
    try:
        with open('/proc/net/udp', 'r') as f:
            for line in f.readlines()[1:]:
                parts = line.split()
                if len(parts) >= 10:
                    local_addr = parts[1]
                    inode = int(parts[9])
                    pid = find_pid_by_inode(inode, 'udp')
                    if pid:
                        mapping[f"udp:{local_addr}"] = pid
    except (FileNotFoundError, ValueError):
        pass
    
    return mapping

def find_pid_by_inode(target_inode: int, socket_type: str) -> Optional[int]:
    """通过 inode 查找 PID（遍历 /proc）"""
    try:
        for pid_dir in os.listdir('/proc'):
            if not pid_dir.isdigit():
                continue
            pid = int(pid_dir)
            fd_dir = f'/proc/{pid}/fd'
            if not os.path.isdir(fd_dir):
                continue
            try:
                for fd in os.listdir(fd_dir):
                    fd_path = f'{fd_dir}/{fd}'
                    try:
                        link = os.readlink(fd_path)
                        # socket:[inode]
                        match = re.match(r'socket:\[(\d+)\]', link)
                        if match and int(match.group(1)) == target_inode:
                            return pid
                    except (OSError, ValueError):
                        continue
            except (OSError, PermissionError):
                continue
    except (OSError, PermissionError):
        pass
    return None

def detect_network_stall(interface: str, stats: Dict, prev_stats: Dict) -> List[Dict]:
    """检测网络阻塞事件"""
    events = []
    current_ts = int(time.time() * 1000)
    
    if interface not in prev_stats:
        return events
    
    prev = prev_stats[interface]
    curr = stats[interface]
    
    # 计算丢包率
    rx_packets_diff = curr['rx_packets'] - prev['rx_packets']
    rx_drops_diff = curr['rx_drops'] - prev['rx_drops']
    
    if rx_packets_diff > 0:
        drop_rate = (rx_drops_diff / rx_packets_diff) * 100
        if drop_rate > 1.0:  # 丢包率超过 1%
            events.append({
                "ts": current_ts,
                "event_type": "transport.drop",
                "entity_id": interface,
                "job_id": None,
                "pid": None,
                "value": f"{drop_rate:.2f}"
            })
    
    # 检测错误
    rx_errors_diff = curr['rx_errors'] - prev['rx_errors']
    tx_errors_diff = curr['tx_errors'] - prev['tx_errors']
    
    if rx_errors_diff > 0 or tx_errors_diff > 0:
        events.append({
            "ts": current_ts,
            "event_type": "error.net",
            "entity_id": interface,
            "job_id": None,
            "pid": None,
            "value": f"RX_ERR:{rx_errors_diff},TX_ERR:{tx_errors_diff}"
        })
    
    return events

def detect_network_bandwidth(interface: str, stats: Dict, prev_stats: Dict, interval: float) -> List[Dict]:
    """检测网络带宽使用"""
    events = []
    current_ts = int(time.time() * 1000)
    
    if interface not in prev_stats:
        return events
    
    prev = prev_stats[interface]
    curr = stats[interface]
    
    # 计算带宽（字节/秒）
    rx_bytes_diff = curr['rx_bytes'] - prev['rx_bytes']
    tx_bytes_diff = curr['tx_bytes'] - prev['tx_bytes']
    
    rx_bw_mbps = (rx_bytes_diff * 8) / (interval * 1_000_000)  # Mbps
    tx_bw_mbps = (tx_bytes_diff * 8) / (interval * 1_000_000)  # Mbps
    
    # 只报告有流量的接口
    if rx_bw_mbps > 0.1 or tx_bw_mbps > 0.1:
        total_bw = rx_bw_mbps + tx_bw_mbps
        events.append({
            "ts": current_ts,
            "event_type": "transport.bw",
            "entity_id": interface,
            "job_id": None,
            "pid": None,
            "value": f"{total_bw:.2f}"
        })
    
    return events

def get_network_io_wait_pids() -> List[int]:
    """检测正在等待网络 I/O 的进程（简化版本）
    
    注意：这是一个启发式方法，实际生产环境应该使用 eBPF 或 strace
    """
    waiting_pids = []
    
    try:
        # 通过 /proc/<pid>/stat 检测进程状态
        # 'D' 状态通常表示等待 I/O（包括网络）
        for pid_dir in os.listdir('/proc'):
            if not pid_dir.isdigit():
                continue
            try:
                pid = int(pid_dir)
                stat_path = f'/proc/{pid}/stat'
                with open(stat_path, 'r') as f:
                    stat_data = f.read()
                parts = stat_data.split()
                if len(parts) > 2:
                    state = parts[2]
                    # 'D' = 不可中断睡眠（通常等待 I/O）
                    # 'S' = 可中断睡眠（可能等待 I/O）
                    if state in ['D', 'S']:
                        # 检查是否有网络相关的文件描述符
                        fd_dir = f'/proc/{pid}/fd'
                        if os.path.isdir(fd_dir):
                            try:
                                for fd in os.listdir(fd_dir):
                                    fd_path = f'{fd_dir}/{fd}'
                                    try:
                                        link = os.readlink(fd_path)
                                        if 'socket' in link:
                                            waiting_pids.append(pid)
                                            break
                                    except (OSError, ValueError):
                                        continue
                            except (OSError, PermissionError):
                                continue
            except (OSError, PermissionError, ValueError):
                continue
    except (OSError, PermissionError):
        pass
    
    return waiting_pids

def generate_events(interval: float) -> List[Dict]:
    """生成所有网络事件"""
    events = []
    current_ts = int(time.time() * 1000)
    
    # 获取网络统计
    stats = parse_net_dev_stats()
    interfaces = list(stats.keys())
    
    # 检测每个接口的阻塞和带宽
    for interface in interfaces:
        # 带宽事件
        events.extend(detect_network_bandwidth(interface, stats, prev_stats, interval))
        
        # 阻塞事件（丢包、错误）
        events.extend(detect_network_stall(interface, stats, prev_stats))
    
    # 检测网络 I/O 等待的进程
    waiting_pids = get_network_io_wait_pids()
    for pid in waiting_pids:
        # 为每个等待网络 I/O 的进程生成事件
        # 这里简化处理，使用第一个活跃接口
        if interfaces:
            interface = interfaces[0]
            events.append({
                "ts": current_ts,
                "event_type": "transport.drop",  # 使用 drop 事件触发 WaitsOn
                "entity_id": interface,
                "job_id": None,
                "pid": pid,
                "value": "IO_WAIT"
            })
    
    # 更新统计
    global prev_stats
    prev_stats = stats
    
    return events

def main():
    """主循环：定期输出网络事件"""
    # 检查是否在 Linux 系统
    if not os.path.exists('/proc/net'):
        print("错误: 此探针需要 Linux 系统（/proc/net 不存在）", file=sys.stderr)
        sys.exit(1)
    
    # 获取采样间隔
    interval = float(os.environ.get("XCTL_NETWORK_INTERVAL", "2.0"))
    
    print(f"网络探针已启动，采样间隔: {interval} 秒", file=sys.stderr)
    
    # 初始化统计
    global prev_stats
    prev_stats = parse_net_dev_stats()
    
    try:
        while True:
            events = generate_events(interval)
            
            # 输出所有事件（JSONL 格式）
            for event in events:
                print(json.dumps(event, ensure_ascii=False))
                sys.stdout.flush()
            
            time.sleep(interval)
            
    except KeyboardInterrupt:
        pass
    except BrokenPipeError:
        # 父进程关闭了管道
        pass

if __name__ == "__main__":
    main()
