# **GMP_for_LLM**

## **队伍介绍**

```
队长：郭睆
队员：包子旭、沈铭
指导老师：张羽教授
学校：西北工业大学
```

## **核心算法说明**

本方案采用基于 **Bélády 最优页面替换算法** 的全局内存调度策略，核心思想如下：

### 1. 算法核心
- **Bélády 预测策略**：在需要卸载内存时，优先卸载"下次使用时间最晚"的内存区域
- **活跃锁定机制**：正在被访问（Visit）的内存区域不能被卸载，确保计算任务正确执行
- **部分卸载优化**：对于与当前请求重叠的内存区域，只卸载不重叠的部分，减少不必要的数据传输
- **Just-in-Time 加载**：在满足 start 时间约束的前提下，尽可能晚地加载数据，为并行执行留出空间

### 2. 并行优化
- **RW/Visit 并行**：内存读写操作（Reload/Offload）可以与访存操作（Visit）并行执行
- **同任务并行**：同一计算任务（start 时间相同）的多个访存请求可以并行处理
- **预加载优化**：利用 Visit 执行时间窗口，提前加载后续请求所需内存

### 3. 内存管理
- **区间合并**：自动合并相邻或重叠的内存区域，减少碎片化
- **精确缺失检测**：只加载 HBM 中不存在的内存段，避免重复加载
- **容量感知调度**：动态计算所需卸载空间，确保不超过 HBM 容量限制

### 4. 性能表现
- **测试覆盖**：3个测试用例均达到最优效果

详细算法设计文档：[GMP_for_LLM 算法设计文档](./GMP_for_LLM%20算法设计文档.pdf)
## **开发环境**

```
Ubuntu 24.04.2 LTS
cat /proc/version
Linux version 6.14.0-29-generic (buildd@lcy02-amd64-105) (x86_64-linux-gnu-gcc-13 (Ubuntu 13.3.0-6ubuntu2~24.04) 13.3.0, GNU ld (GNU Binutils for Ubuntu) 2.42) #29~24.04.1-Ubuntu SMP PREEMPT_DYNAMIC Thu Aug 14 16:52:50 UTC 2
rustup --version
rustup 1.28.2 (e4f3ad6f8 2025-04-28)
cargo --version
cargo 1.91.0 (ea2d97820 2025-10-10)
rustc --version
rustc 1.91.0 (f8297e351 2025-10-28)
```
## **快速开始**

```
自行输入：
cargo run
示例评测：
cargo run test
从infile.txt读取，输出到outfile.txt：
cargo run < infile.txt > outfile.txt
运行checker:
./checker/checker infile.txt outfile.txt outfile.txt
```

