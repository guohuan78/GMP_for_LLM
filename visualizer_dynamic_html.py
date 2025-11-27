#!/usr/bin/env python3
"""
生成动态 HTML 可视化，支持步进控制
"""

import re
import sys
from typing import List, Tuple, Dict
import json


class MemoryEvent:
    def __init__(self, timestamp: int):
        self.timestamp = timestamp


class ReloadEvent(MemoryEvent):
    def __init__(self, timestamp: int, addr: int, size: int):
        super().__init__(timestamp)
        self.addr = addr
        self.size = size


class OffloadEvent(MemoryEvent):
    def __init__(self, timestamp: int, addr: int, size: int):
        super().__init__(timestamp)
        self.addr = addr
        self.size = size


class VisitEvent(MemoryEvent):
    def __init__(self, timestamp: int, request_id: int):
        super().__init__(timestamp)
        self.request_id = request_id


class FinEvent(MemoryEvent):
    def __init__(self, timestamp: int):
        super().__init__(timestamp)


def parse_output(lines: List[str]) -> List[MemoryEvent]:
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


def generate_dynamic_html(events: List[MemoryEvent], output_file: str, hbm_capacity: int = 300):
    # 找出虚拟地址空间最大值
    max_vaddr = 0
    for event in events:
        if isinstance(event, ReloadEvent):
            max_vaddr = max(max_vaddr, event.addr + event.size)
    
    # 生成每一步的状态
    states = []
    regions = {}  # {addr: size}
    
    for event in events:
        event_type = ""
        event_desc = ""
        
        if isinstance(event, ReloadEvent):
            regions[event.addr] = event.size
            event_type = "reload"
            event_desc = f"加载虚拟地址 [{event.addr}, {event.addr + event.size}) 共 {event.size} 字节"
        elif isinstance(event, OffloadEvent):
            # 卸载虚拟地址范围 [event.addr, event.addr + event.size)
            offload_end = event.addr + event.size
            regions_to_remove = []
            regions_to_add = {}
            
            for region_start, region_size in list(regions.items()):
                region_end = region_start + region_size
                
                # 检查卸载范围与当前区域是否有重叠
                if event.addr < region_end and offload_end > region_start:
                    overlap_start = max(event.addr, region_start)
                    overlap_end = min(offload_end, region_end)
                    
                    regions_to_remove.append(region_start)
                    
                    # 保留卸载范围之前的部分
                    if region_start < overlap_start:
                        regions_to_add[region_start] = overlap_start - region_start
                    
                    # 保留卸载范围之后的部分
                    if region_end > overlap_end:
                        regions_to_add[overlap_end] = region_end - overlap_end
            
            for region_start in regions_to_remove:
                del regions[region_start]
            regions.update(regions_to_add)
            
            event_type = "offload"
            event_desc = f"卸载虚拟地址 [{event.addr}, {event.addr + event.size}) 共 {event.size} 字节"
        elif isinstance(event, VisitEvent):
            event_type = "visit"
            event_desc = f"访问请求 R{event.request_id}"
        elif isinstance(event, FinEvent):
            event_type = "fin"
            event_desc = "所有请求完成"
        
        usage = sum(regions.values())
        
        states.append({
            'timestamp': event.timestamp,
            'type': event_type,
            'desc': event_desc,
            'usage': usage,
            'regions': list(sorted(regions.items()))
        })
    
    # 生成 HTML
    html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>HBM 动态可视化</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: 'Consolas', 'Monaco', monospace;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            color: #e0e0e0;
            padding: 20px;
            min-height: 100vh;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}
        h1 {{
            text-align: center;
            color: #00d4ff;
            margin-bottom: 10px;
            font-size: 32px;
            text-shadow: 0 0 20px rgba(0, 212, 255, 0.5);
        }}
        .subtitle {{
            text-align: center;
            color: #888;
            margin-bottom: 30px;
        }}
        .controls {{
            background: rgba(37, 37, 37, 0.8);
            padding: 20px;
            border-radius: 12px;
            margin-bottom: 20px;
            display: flex;
            gap: 15px;
            align-items: center;
            flex-wrap: wrap;
            border: 1px solid #333;
        }}
        button {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 8px;
            cursor: pointer;
            font-size: 14px;
            font-weight: bold;
            transition: all 0.3s;
            box-shadow: 0 4px 15px rgba(102, 126, 234, 0.4);
        }}
        button:hover {{
            transform: translateY(-2px);
            box-shadow: 0 6px 20px rgba(102, 126, 234, 0.6);
        }}
        button:active {{
            transform: translateY(0);
        }}
        button:disabled {{
            background: #555;
            cursor: not-allowed;
            box-shadow: none;
        }}
        .step-info {{
            flex: 1;
            min-width: 200px;
            text-align: center;
            font-size: 16px;
            color: #00d4ff;
            font-weight: bold;
        }}
        .speed-control {{
            display: flex;
            align-items: center;
            gap: 10px;
        }}
        input[type="range"] {{
            width: 150px;
        }}
        .main-display {{
            background: rgba(37, 37, 37, 0.8);
            padding: 30px;
            border-radius: 12px;
            border: 1px solid #333;
        }}
        .event-banner {{
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 25px;
            font-size: 18px;
            font-weight: bold;
            text-align: center;
            border-left: 5px solid;
            transition: all 0.3s;
        }}
        .event-reload {{
            background: rgba(0, 255, 136, 0.1);
            border-color: #00ff88;
            color: #00ff88;
        }}
        .event-offload {{
            background: rgba(255, 107, 53, 0.1);
            border-color: #ff6b35;
            color: #ff6b35;
        }}
        .event-visit {{
            background: rgba(157, 78, 221, 0.1);
            border-color: #9d4edd;
            color: #9d4edd;
        }}
        .event-fin {{
            background: rgba(6, 255, 165, 0.1);
            border-color: #06ffa5;
            color: #06ffa5;
        }}
        .usage-bar {{
            margin-bottom: 30px;
        }}
        .usage-label {{
            display: flex;
            justify-content: space-between;
            margin-bottom: 10px;
            font-size: 16px;
        }}
        .usage-text {{
            color: #00d4ff;
            font-weight: bold;
        }}
        .progress-container {{
            height: 30px;
            background: #2a2a2a;
            border-radius: 15px;
            overflow: hidden;
            position: relative;
            border: 2px solid #444;
        }}
        .progress-bar {{
            height: 100%;
            background: linear-gradient(90deg, #00ff88, #00d4ff);
            transition: width 0.5s ease;
            display: flex;
            align-items: center;
            justify-content: center;
            color: white;
            font-weight: bold;
            box-shadow: 0 0 20px rgba(0, 255, 136, 0.5);
        }}
        .memory-viz {{
            margin-bottom: 30px;
        }}
        .memory-label {{
            margin-bottom: 15px;
            font-size: 16px;
            color: #00d4ff;
            font-weight: bold;
        }}
        .memory-bar {{
            height: 40px;
            background: #2a2a2a;
            border-radius: 8px;
            position: relative;
            border: 2px solid #444;
            overflow: hidden;
        }}
        .memory-segment {{
            position: absolute;
            height: 100%;
            background: linear-gradient(180deg, #00ff88, #00cc70);
            border-right: 2px solid #1a1a1a;
            transition: all 0.5s ease;
        }}
        .memory-segment:hover {{
            filter: brightness(1.2);
            z-index: 10;
        }}
        .regions-list {{
            background: #1a1a1a;
            padding: 20px;
            border-radius: 8px;
            border: 1px solid #333;
        }}
        .regions-title {{
            color: #00d4ff;
            font-weight: bold;
            margin-bottom: 15px;
            font-size: 16px;
        }}
        .region-item {{
            padding: 10px;
            margin: 8px 0;
            background: rgba(0, 212, 255, 0.05);
            border-left: 3px solid #00d4ff;
            border-radius: 4px;
            font-family: monospace;
            transition: all 0.3s;
        }}
        .region-item:hover {{
            background: rgba(0, 212, 255, 0.1);
            transform: translateX(5px);
        }}
        .tooltip {{
            position: absolute;
            background: rgba(0, 0, 0, 0.9);
            color: white;
            padding: 8px 12px;
            border-radius: 6px;
            font-size: 12px;
            pointer-events: none;
            z-index: 1000;
            display: none;
            border: 1px solid #00d4ff;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🧠 HBM 虚拟内存动态可视化</h1>
        <div class="subtitle">逐步查看虚拟地址空间在 HBM 中的加载过程</div>
        
        <div class="controls">
            <button onclick="firstStep()">⏮ 第一步</button>
            <button onclick="prevStep()">◀ 上一步</button>
            <button onclick="playPause()" id="playBtn">▶ 播放</button>
            <button onclick="nextStep()">下一步 ▶</button>
            <button onclick="lastStep()">最后一步 ⏭</button>
            
            <div class="step-info" id="stepInfo">步骤 1/{len(states)}</div>
            
            <div class="speed-control">
                <label>速度:</label>
                <input type="range" id="speedSlider" min="100" max="2000" value="1000" step="100">
                <span id="speedLabel">1.0x</span>
            </div>
        </div>
        
        <div class="main-display">
            <div class="event-banner" id="eventBanner">
                准备开始...
            </div>
            
            <div class="usage-bar">
                <div class="usage-label">
                    <span class="usage-text">HBM 使用量</span>
                    <span id="usageText">0 / {hbm_capacity} (0.0%)</span>
                </div>
                <div class="progress-container">
                    <div class="progress-bar" id="progressBar" style="width: 0%">0%</div>
                </div>
            </div>
            
            <div class="memory-viz">
                <div class="memory-label">虚拟地址空间 [0 ~ {max_vaddr}]</div>
                <div class="memory-bar" id="memoryBar"></div>
            </div>
            
            <div class="regions-list">
                <div class="regions-title">HBM 中的虚拟内存区域</div>
                <div id="regionsList">
                    <div style="color: #888; text-align: center; padding: 20px;">
                        (空)
                    </div>
                </div>
            </div>
        </div>
    </div>
    
    <div class="tooltip" id="tooltip"></div>
    
    <script>
        const states = {json.dumps(states)};
        const maxVaddr = {max_vaddr};
        const hbmCapacity = {hbm_capacity};
        
        let currentStep = 0;
        let isPlaying = false;
        let playInterval = null;
        
        function updateDisplay() {{
            if (currentStep < 0 || currentStep >= states.length) return;
            
            const state = states[currentStep];
            
            // 更新步骤信息
            document.getElementById('stepInfo').textContent = 
                `步骤 ${{currentStep + 1}}/${{states.length}} | 时间: ${{state.timestamp}}`;
            
            // 更新事件横幅
            const banner = document.getElementById('eventBanner');
            banner.textContent = state.desc;
            banner.className = 'event-banner event-' + state.type;
            
            // 更新使用量
            const usage = state.usage;
            const percentage = (usage / hbmCapacity * 100).toFixed(1);
            document.getElementById('usageText').textContent = 
                `${{usage}} / ${{hbmCapacity}} (${{percentage}}%)`;
            
            const progressBar = document.getElementById('progressBar');
            progressBar.style.width = percentage + '%';
            progressBar.textContent = percentage + '%';
            
            // 更新内存条
            const memoryBar = document.getElementById('memoryBar');
            memoryBar.innerHTML = '';
            
            state.regions.forEach(([addr, size]) => {{
                const segment = document.createElement('div');
                segment.className = 'memory-segment';
                const left = (addr / maxVaddr * 100).toFixed(2);
                const width = (size / maxVaddr * 100).toFixed(2);
                segment.style.left = left + '%';
                segment.style.width = width + '%';
                segment.title = `[${{addr}}, ${{addr + size}}) = ${{size}} 字节`;
                
                segment.addEventListener('mouseenter', (e) => {{
                    const tooltip = document.getElementById('tooltip');
                    tooltip.textContent = segment.title;
                    tooltip.style.display = 'block';
                }});
                
                segment.addEventListener('mousemove', (e) => {{
                    const tooltip = document.getElementById('tooltip');
                    tooltip.style.left = e.pageX + 10 + 'px';
                    tooltip.style.top = e.pageY + 10 + 'px';
                }});
                
                segment.addEventListener('mouseleave', () => {{
                    document.getElementById('tooltip').style.display = 'none';
                }});
                
                memoryBar.appendChild(segment);
            }});
            
            // 更新区域列表
            const regionsList = document.getElementById('regionsList');
            if (state.regions.length === 0) {{
                regionsList.innerHTML = '<div style="color: #888; text-align: center; padding: 20px;">(空)</div>';
            }} else {{
                regionsList.innerHTML = state.regions.map(([addr, size]) => 
                    `<div class="region-item">[${{addr.toString().padStart(4)}},${{(addr + size).toString().padStart(4)}}) = ${{size.toString().padStart(3)}} 字节</div>`
                ).join('');
            }}
        }}
        
        function firstStep() {{
            currentStep = 0;
            updateDisplay();
        }}
        
        function lastStep() {{
            currentStep = states.length - 1;
            updateDisplay();
        }}
        
        function prevStep() {{
            if (currentStep > 0) {{
                currentStep--;
                updateDisplay();
            }}
        }}
        
        function nextStep() {{
            if (currentStep < states.length - 1) {{
                currentStep++;
                updateDisplay();
            }} else {{
                pause();
            }}
        }}
        
        function play() {{
            isPlaying = true;
            document.getElementById('playBtn').textContent = '⏸ 暂停';
            const speed = parseInt(document.getElementById('speedSlider').value);
            playInterval = setInterval(() => {{
                nextStep();
            }}, speed);
        }}
        
        function pause() {{
            isPlaying = false;
            document.getElementById('playBtn').textContent = '▶ 播放';
            if (playInterval) {{
                clearInterval(playInterval);
                playInterval = null;
            }}
        }}
        
        function playPause() {{
            if (isPlaying) {{
                pause();
            }} else {{
                play();
            }}
        }}
        
        // 速度滑块
        document.getElementById('speedSlider').addEventListener('input', (e) => {{
            const speed = parseInt(e.target.value);
            const speedFactor = (2100 - speed) / 1000;
            document.getElementById('speedLabel').textContent = speedFactor.toFixed(1) + 'x';
            
            if (isPlaying) {{
                pause();
                play();
            }}
        }});
        
        // 键盘快捷键
        document.addEventListener('keydown', (e) => {{
            if (e.key === 'ArrowLeft') prevStep();
            else if (e.key === 'ArrowRight') nextStep();
            else if (e.key === ' ') {{
                e.preventDefault();
                playPause();
            }}
            else if (e.key === 'Home') firstStep();
            else if (e.key === 'End') lastStep();
        }});
        
        // 初始化
        updateDisplay();
    </script>
</body>
</html>
"""
    
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(html)
    
    print(f"✅ 动态 HTML 可视化已保存到: {output_file}")
    print(f"   总步骤数: {len(states)}")
    print(f"   虚拟地址空间: [0, {max_vaddr})")
    print(f"   HBM 容量: {hbm_capacity}")


def main():
    if len(sys.argv) < 2:
        print("用法: python3 visualizer_dynamic_html.py <output_file> [hbm_capacity]")
        print("示例: python3 visualizer_dynamic_html.py outfile.txt 300")
        return
    
    input_file = sys.argv[1]
    
    with open(input_file, 'r') as f:
        lines = f.readlines()
    
    # 尝试从对应的 infile.txt 读取 HBM 容量
    hbm_capacity = 300  # 默认值
    infile_path = input_file.replace('outfile', 'infile')
    try:
        with open(infile_path, 'r') as f:
            first_line = f.readline().strip().split()
            if len(first_line) >= 2:
                hbm_capacity = int(first_line[1])  # m (HBM capacity)
                print(f"从 {infile_path} 读取 HBM 容量: {hbm_capacity}")
    except:
        pass
    
    # 命令行参数可以覆盖
    if len(sys.argv) > 2:
        hbm_capacity = int(sys.argv[2])
    
    events = parse_output(lines)
    
    if not events:
        print("错误: 没有解析到任何事件")
        return
    
    output_file = input_file.replace('.txt', '_dynamic.html')
    generate_dynamic_html(events, output_file, hbm_capacity)


if __name__ == "__main__":
    main()
