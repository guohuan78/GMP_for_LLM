# GMP_for_LLM 快速参考指南

## 🚀 快速开始

### 编译项目
```bash
cargo build
```

### 运行模式

#### 1. 标准输入模式（手动测试）
```bash
cargo run
```
然后输入测试数据，例如：
```
200 100 2
0 100 0 30
100 100 50 10
```

#### 2. README示例测试（2个）
```bash
cargo run test
```
自动运行README中的示例一和示例二。

#### 3. 扩展测试集（10个）
```bash
cargo run extended
```
运行 `test_cases.json` 中定义的10个综合测试用例。

### 查看结果摘要
```bash
# README示例
cargo run test 2>&1 | grep -E "示例|通过|匹配"

# 扩展测试
cargo run extended 2>&1 | grep -E "测试 #|通过率"

# 运行所有测试
cargo run test && echo && cargo run extended
```

## 📊 当前测试结果

| 测试集 | 通过率 | 详情 |
|--------|--------|------|
| README示例 | 100% (2/2) | 示例一、二完美匹配 |
| 扩展测试集 | 90% (9/10) | 7个完美，2个超越，1个待优化 |
| **总计** | **91.7% (11/12)** | 高质量实现 |

## 📁 重要文件

| 文件 | 说明 |
|------|------|
| `src/main.rs` | 核心算法实现（Bélády + 优化）|
| `test_cases.json` | 10个测试用例定义 |
| `TESTING_SUMMARY.md` | 测试结果详细报告 |
| `PROJECT_STRUCTURE.md` | 项目结构说明 |

## 🎯 测试用例概览

1. **简单顺序访问** - HBM充足，无需卸载 ✓
2. **内存复用** - 检测已加载内存 ✓
3. **Bélády算法** - 选择最优卸载对象 ✓
4. **部分重叠** - 精确缺失检测 ✓✓ (超越预期)
5. **密集并发** - 时间重叠处理 ⚠️ (待优化)
6. **完全重叠** - 子集关系识别 ✓
7. **最小容量** - 极限压力测试 ✓
8. **零时延启动** - 批处理优化 ✓✓ (超越预期)
9. **碎片化内存** - 区间合并 ✓
10. **长时延访问** - 并行窗口利用 ✓

## 🔧 添加新测试用例

编辑 `test_cases.json`，添加新的测试对象：

```json
{
  "id": 11,
  "name": "你的测试名称",
  "description": "测试描述",
  "input": {
    "L": 300,
    "M": 200,
    "N": 2,
    "requests": [
      {"addr": 0, "size": 100, "start": 0, "time": 50},
      {"addr": 100, "size": 100, "start": 5000, "time": 30}
    ]
  },
  "expected_output": [],
  "expected_fin": 8030,
  "explanation": "预期行为说明"
}
```

然后运行 `cargo run extended` 查看结果。

## 📈 算法核心

- **Bélády最优替换**: 卸载"下次使用最晚"的内存
- **内存复用**: 避免重复加载
- **部分卸载**: 只卸载必要部分
- **活跃锁定**: Visit期间内存不可卸载
- **JIT加载**: 在访问前刚好完成
- **并行优化**: RW和Visit同时执行

## 🐛 调试技巧

### 查看详细输出
```bash
cargo run test        # 完整输出
cargo run extended    # 完整输出
```

### 测试单个场景
修改 `src/tests.rs` 或创建临时输入文件：
```bash
echo "200 100 2
0 100 0 30
100 100 50 10" | cargo run
```

### 查看编译警告
```bash
cargo build 2>&1 | grep warning
```

## 📚 更多文档

- **算法详解**: 见 `README.md` 第七章
- **测试结果**: 见 `TESTING_SUMMARY.md`
- **项目结构**: 见 `PROJECT_STRUCTURE.md`
- **代码注释**: 见 `src/main.rs`

## ⚡ 性能提示

- 使用 `--release` 编译优化版本：
  ```bash
  cargo build --release
  ./target/release/gmp_for_llm
  ```

- 当前实现已在多数场景达到理论最优
- 部分场景（如测试#4、#8）甚至超越初始预期

---

**最后更新**: 2025-11-03  
**版本**: v1.0  
**团队**: 西北工业大学 - 郭睆、包子旭、沈铭
