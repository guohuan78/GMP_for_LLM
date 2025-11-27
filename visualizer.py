#!/usr/bin/env python3
"""
GMP for LLM - 内存调度可视化工具
使用图形化方式展示 HBM 内存状态
"""

import re
import sys
from typing import List, Tuple, Dict


class MemoryEvent:
    """内存事件基类"""
    def __init__(self, timestamp: int):
        self.timestamp = timestamp


class ReloadEvent(MemoryEvent):
    """Reload事件"""
    def __init__(self, timestamp: int, addr: int, size: int):
        super().__init__(timestamp)
        self.addr = addr
        self.size = size


class OffloadEvent(MemoryEvent):
    """Offload事件"""
    def __init__(self, timestamp: int, addr: int, size: int):
        super().__init__(timestamp)
        self.addr = addr
        self.size = size


class VisitEvent(MemoryEvent):
    """Visit事件"""
    def __init__(self, timestamp: int, request_id: int):
        super().__init__(timestamp)
        self.request_id = request_id


class FinEvent(MemoryEvent):
    """Fin事件"""
    def __init__(self, timestamp: int):
        super().__init__(timestamp)


def parse_output(lines: List[str]) -> List[MemoryEvent]:
    """解析输出文件"""
    events = []
    
    for line in lines:
        line = line.strip()
        if not line:
            continue
            
        if line.startswith("Reload"):
            match = re.match(r'Reload\s+(\d+)\s+(\d+)\s+(\d+)', line)
            if match:
                timestamp, addr, size = map(int, match.groups())
                events.append(ReloadEvent(timestamp, addr, size))
                
        elif line.startswith("Offload"):
            match = re.match(r'Offload\s+(\d+)\s+(\d+)\s+(\d+)', line)
            if match:
                timestamp, addr, size = map(int, match.groups())
                events.append(OffloadEvent(timestamp, addr, size))
                
        elif line.startswith("Visit"):
            match = re.match(r'Visit\s+(\d+)\s+(\d+)', line)
            if match:
                timestamp, request_id = map(int, match.groups())
                events.append(VisitEvent(timestamp, request_id))
                
        elif line.startswith("Fin"):
            match = re.match(r'Fin\s+(\d+)', line)
            if match:
                timestamp = int(match.group(1))
                events.append(FinEvent(timestamp))
    
    return events


def visualize_memory_blocks(state: Dict[int, int], max_addr: int, bar_width: int = 50) -> Tuple[str, str]:
    """
    图形化显示内存块分布
    返回 (图形行, 标签行)
    """
    if not state:
        return "░" * bar_width, " " * bar_width
    
    # 创建内存映射
    memory_map = ['░'] * bar_width  # 空闲用 ░
    labels = [' '] * bar_width
    
    # 计算比例
    scale = max_addr / bar_width if max_addr > 0 else 1
    
    # 标记已占用的内存
    for addr, size in sorted(state.items()):
        start_pos = int(addr / scale)
        end_pos = int((addr + size) / scale)
        
        # 确保至少显示1个字符
        if start_pos == end_pos:
            end_pos = start_pos + 1
        
        # 限制范围
        start_pos = max(0, min(start_pos, bar_width - 1))
        end_pos = max(0, min(end_pos, bar_width))
        
        # 填充内存块
        for i in range(start_pos, end_pos):
            memory_map[i] = '█'
        
        # 添加地址标签（在起始位置）
        label = str(addr)
        if start_pos + len(label) <= bar_width:
            for i, ch in enumerate(label):
                if start_pos + i < bar_width:
                    labels[start_pos + i] = ch
    
    return ''.join(memory_map), ''.join(labels)


def visualize_memory_state(events: List[MemoryEvent]):
    """可视化 HBM 内存状态变化"""
    
    if not events:
        print("没有事件可以可视化")
        return
    
    # 找出最大地址用于缩放
    max_addr = 0
    for event in events:
        if isinstance(event, ReloadEvent):
            max_addr = max(max_addr, event.addr + event.size)
    
    # 跟踪内存状态 - 使用区间来正确处理部分卸载
    # 格式: {(start, end): True} 表示 [start, end) 区间被占用
    current_blocks = {}
    
    print("\n" + "=" * 110)
    print(" " * 40 + "HBM 内存状态可视化")
    print("=" * 110)
    print(f"{'时间':^8} | {'操作':^14} | {'详情':^18} | {'占用':^6} | 内存块分布 (0 ~ {max_addr})")
    print("-" * 110)
    
    for event in sorted(events, key=lambda e: e.timestamp):
        ts = event.timestamp
        
        if isinstance(event, ReloadEvent):
            # 加载：添加新的内存块
            current_blocks[event.addr] = event.size
            op = "⬆ 加载"
            detail = f"[{event.addr}+{event.size}]"
            
        elif isinstance(event, OffloadEvent):
            # 卸载：需要处理部分卸载的情况
            offload_start = event.addr
            offload_end = event.addr + event.size
            
            # 找出所有与卸载区间重叠的内存块
            blocks_to_update = []
            for block_addr, block_size in list(current_blocks.items()):
                block_end = block_addr + block_size
                
                # 检查是否有重叠
                if not (block_end <= offload_start or block_addr >= offload_end):
                    blocks_to_update.append((block_addr, block_size))
            
            # 处理每个重叠的块
            for block_addr, block_size in blocks_to_update:
                block_end = block_addr + block_size
                
                # 删除原块
                del current_blocks[block_addr]
                
                # 计算剩余部分
                # 左边剩余部分: [block_addr, offload_start)
                if block_addr < offload_start:
                    left_size = offload_start - block_addr
                    current_blocks[block_addr] = left_size
                
                # 右边剩余部分: [offload_end, block_end)
                if block_end > offload_end:
                    right_size = block_end - offload_end
                    current_blocks[offload_end] = right_size
            
            op = "⬇ 卸载"
            detail = f"[{event.addr}+{event.size}]"
            
        elif isinstance(event, VisitEvent):
            op = f"✓ 访问 R{event.request_id}"
            detail = ""
            
        elif isinstance(event, FinEvent):
            op = "★ 完成"
            detail = ""
        else:
            continue
        
        # 计算当前 HBM 总占用
        total = sum(current_blocks.values())
        
        # 生成图形化内存块显示
        bar, labels = visualize_memory_blocks(current_blocks, max_addr, bar_width=50)
        
        # 打印事件行和内存块图形
        print(f"{ts:8d} | {op:^14} | {detail:^18} | {total:6d} | {bar}")
        if labels.strip():  # 只有当有标签时才打印
            print(f"{'':8} | {'':^14} | {'':^18} | {'':^6} | {labels}")
    
    print("=" * 110)


def visualize_html(events: List[MemoryEvent], output_file: str = "memory_timeline.html"):
    """生成 HTML 可视化"""
    
    if not events:
        print("没有事件可以可视化")
        return
    
    # 找出最大地址
    max_addr = 0
    for event in events:
        if isinstance(event, ReloadEvent):
            max_addr = max(max_addr, event.addr + event.size)
    
    # 跟踪内存状态
    memory_history = []
    current_blocks = {}
    
    for event in sorted(events, key=lambda e: e.timestamp):
        if isinstance(event, ReloadEvent):
            current_blocks[event.addr] = event.size
            
        elif isinstance(event, OffloadEvent):
            # 卸载：处理部分卸载
            offload_start = event.addr
            offload_end = event.addr + event.size
            
            blocks_to_update = []
            for block_addr, block_size in list(current_blocks.items()):
                block_end = block_addr + block_size
                if not (block_end <= offload_start or block_addr >= offload_end):
                    blocks_to_update.append((block_addr, block_size))
            
            for block_addr, block_size in blocks_to_update:
                block_end = block_addr + block_size
                del current_blocks[block_addr]
                
                if block_addr < offload_start:
                    current_blocks[block_addr] = offload_start - block_addr
                
                if block_end > offload_end:
                    current_blocks[offload_end] = block_end - offload_end
        
        memory_history.append((event.timestamp, dict(current_blocks), event))
    
    # 生成 HTML
    html = """<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>HBM 内存状态可视化</title>
    <style>
        body { 
            font-family: 'Consolas', 'Monaco', monospace; 
            background: #1a1a1a; 
            color: #e0e0e0; 
            padding: 20px;
            margin: 0;
        }
        h1 { 
            text-align: center; 
            color: #00d4ff;
            margin: 20px 0;
            font-size: 28px;
        }
        .container {
            max-width: 1400px;
            margin: 0 auto;
        }
        .stats {
            background: #252525;
            padding: 20px;
            border-radius: 8px;
            margin: 20px 0;
            border-left: 4px solid #00d4ff;
        }
        .stats h2 {
            color: #00d4ff;
            margin: 0 0 15px 0;
            font-size: 20px;
        }
        .stat-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 15px;
        }
        .stat-item {
            background: #1a1a1a;
            padding: 12px;
            border-radius: 4px;
            border: 1px solid #333;
        }
        .stat-label {
            color: #888;
            font-size: 12px;
            margin-bottom: 5px;
        }
        .stat-value {
            color: #00ff88;
            font-size: 24px;
            font-weight: bold;
        }
        .timeline {
            background: #252525;
            padding: 15px;
            border-radius: 8px;
            margin: 20px 0;
        }
        .event {
            display: grid;
            grid-template-columns: 80px 130px 180px 80px 1fr;
            gap: 15px;
            padding: 12px;
            margin: 8px 0;
            background: #1a1a1a;
            border-radius: 4px;
            border-left: 4px solid #444;
            align-items: center;
        }
        .event.reload { border-left-color: #00ff88; }
        .event.offload { border-left-color: #ff6b35; }
        .event.visit { border-left-color: #9d4edd; }
        .event.fin { border-left-color: #06ffa5; background: #1a2a1a; }
        .time {
            color: #00d4ff;
            font-weight: bold;
            font-size: 14px;
        }
        .operation {
            font-weight: bold;
            font-size: 14px;
        }
        .reload-op { color: #00ff88; }
        .offload-op { color: #ff6b35; }
        .visit-op { color: #9d4edd; }
        .fin-op { color: #06ffa5; }
        .detail {
            color: #ffd60a;
            font-family: monospace;
        }
        .hbm-usage {
            color: #00d4ff;
            font-weight: bold;
            text-align: right;
        }
        .memory-bar-container {
            position: relative;
        }
        .memory-bar {
            height: 24px;
            background: #2a2a2a;
            border-radius: 4px;
            position: relative;
            overflow: hidden;
            border: 1px solid #444;
        }
        .memory-segment {
            position: absolute;
            height: 100%;
            background: linear-gradient(180deg, #00ff88, #00cc70);
            border-right: 1px solid #1a1a1a;
            box-sizing: border-box;
        }
        .memory-labels {
            font-size: 10px;
            color: #888;
            margin-top: 2px;
            height: 14px;
            position: relative;
        }
        .memory-label {
            position: absolute;
            color: #00d4ff;
            font-size: 9px;
        }
        .header-row {
            display: grid;
            grid-template-columns: 80px 130px 180px 80px 1fr;
            gap: 15px;
            padding: 12px;
            background: #2a2a2a;
            border-radius: 4px;
            margin-bottom: 10px;
            font-weight: bold;
            color: #00d4ff;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🧠 HBM 内存状态可视化</h1>
"""
    
    # 统计信息
    reload_count = sum(1 for e in events if isinstance(e, ReloadEvent))
    offload_count = sum(1 for e in events if isinstance(e, OffloadEvent))
    visit_count = sum(1 for e in events if isinstance(e, VisitEvent))
    reload_total = sum(e.size for e in events if isinstance(e, ReloadEvent))
    offload_total = sum(e.size for e in events if isinstance(e, OffloadEvent))
    fin_time = next((e.timestamp for e in events if isinstance(e, FinEvent)), 0)
    
    html += f"""
        <div class="stats">
            <h2>📊 统计信息</h2>
            <div class="stat-grid">
                <div class="stat-item">
                    <div class="stat-label">总完成时间</div>
                    <div class="stat-value">{fin_time}</div>
                </div>
                <div class="stat-item">
                    <div class="stat-label">Reload 次数 / 总量</div>
                    <div class="stat-value">{reload_count} / {reload_total}</div>
                </div>
                <div class="stat-item">
                    <div class="stat-label">Offload 次数 / 总量</div>
                    <div class="stat-value">{offload_count} / {offload_total}</div>
                </div>
                <div class="stat-item">
                    <div class="stat-label">Visit 次数</div>
                    <div class="stat-value">{visit_count}</div>
                </div>
            </div>
        </div>
        
        <div class="timeline">
            <div class="header-row">
                <div>时间</div>
                <div>操作</div>
                <div>详情</div>
                <div style="text-align: right;">HBM占用</div>
                <div>内存块分布 (0 ~ {max_addr})</div>
            </div>
"""
    
    for ts, state, event in memory_history:
        if isinstance(event, ReloadEvent):
            event_class = "reload"
            op_class = "reload-op"
            operation = "⬆ 加载"
            detail = f"[{event.addr}+{event.size}]"
        elif isinstance(event, OffloadEvent):
            event_class = "offload"
            op_class = "offload-op"
            operation = "⬇ 卸载"
            detail = f"[{event.addr}+{event.size}]"
        elif isinstance(event, VisitEvent):
            event_class = "visit"
            op_class = "visit-op"
            operation = f"✓ 访问 R{event.request_id}"
            detail = ""
        elif isinstance(event, FinEvent):
            event_class = "fin"
            op_class = "fin-op"
            operation = "★ 完成"
            detail = ""
        else:
            continue
        
        total_memory = sum(state.values())
        
        # 生成内存条图形
        segments_html = ""
        labels_html = ""
        for addr, size in sorted(state.items()):
            left_percent = (addr / max_addr * 100) if max_addr > 0 else 0
            width_percent = (size / max_addr * 100) if max_addr > 0 else 0
            segments_html += f'<div class="memory-segment" style="left: {left_percent:.2f}%; width: {width_percent:.2f}%;"></div>'
            labels_html += f'<span class="memory-label" style="left: {left_percent:.2f}%;">{addr}</span>'
        
        if not segments_html:
            segments_html = '<div style="text-align: center; line-height: 24px; color: #666;">(空)</div>'
        
        html += f"""
            <div class="event {event_class}">
                <div class="time">{ts}</div>
                <div class="operation {op_class}">{operation}</div>
                <div class="detail">{detail}</div>
                <div class="hbm-usage">{total_memory}</div>
                <div class="memory-bar-container">
                    <div class="memory-bar">{segments_html}</div>
                    <div class="memory-labels">{labels_html}</div>
                </div>
            </div>
"""
    
    html += """
        </div>
    </div>
</body>
</html>
"""
    
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(html)
    
    print(f"\n✅ HTML可视化已保存到: {output_file}")


def print_statistics(events: List[MemoryEvent]):
    """打印统计信息"""
    reload_count = sum(1 for e in events if isinstance(e, ReloadEvent))
    offload_count = sum(1 for e in events if isinstance(e, OffloadEvent))
    visit_count = sum(1 for e in events if isinstance(e, VisitEvent))
    
    reload_total_size = sum(e.size for e in events if isinstance(e, ReloadEvent))
    offload_total_size = sum(e.size for e in events if isinstance(e, OffloadEvent))
    
    fin_time = next((e.timestamp for e in events if isinstance(e, FinEvent)), 0)
    
    print("\n" + "=" * 60)
    print(" " * 22 + "统计信息")
    print("=" * 60)
    print(f"  总完成时间:     {fin_time}")
    print(f"  Reload 次数:    {reload_count} (总量: {reload_total_size})")
    print(f"  Offload 次数:   {offload_count} (总量: {offload_total_size})")
    print(f"  Visit 次数:     {visit_count}")
    print("=" * 60)


def main():
    """主函数"""
    if len(sys.argv) > 1:
        input_file = sys.argv[1]
        with open(input_file, 'r') as f:
            lines = f.readlines()
    else:
        print("用法: python3 visualizer.py <output_file>")
        print("示例: python3 visualizer.py outfile.txt")
        return
    
    # 解析事件
    events = parse_output(lines)
    
    if not events:
        print("错误: 没有解析到任何事件")
        return
    
    # 打印统计信息
    print_statistics(events)
    
    # 文本可视化 - 图形化展示内存块
    visualize_memory_state(events)
    
    # 生成 HTML 可视化
    html_file = input_file.replace('.txt', '.html')
    visualize_html(events, html_file)


if __name__ == "__main__":
    main()
