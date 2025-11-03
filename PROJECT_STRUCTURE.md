# GMP_for_LLM 项目结构

```
GMP_for_LLM/
│
├── Cargo.toml                      # Rust项目配置文件
├── Cargo.lock                      # 依赖锁定文件
│
├── README.md                       # 项目主文档（赛题说明）
├── TEST_RESULTS.md                 # 测试结果报告
├── TEST_CASES_EXPLANATION.md       # 测试用例详细说明
├── PROJECT_STRUCTURE.md            # 本文件（项目结构说明）
│
├── test_cases.json                 # 扩展测试用例数据（JSON格式）
│
├── src/
│   ├── main.rs                     # 主程序入口 + 核心算法实现
│   ├── tests.rs                    # README示例测试框架
│   └── extended_tests.rs           # 扩展测试用例运行器
│
└── target/                         # Cargo编译输出目录（自动生成）
    └── debug/
        └── gmp_for_llm             # 可执行文件
```

## 文件说明

### 核心代码文件

#### `src/main.rs` (307行)
**功能**: 核心调度算法实现
- **数据结构**:
  - `Req`: 内存访问请求（id, addr, size, start, time）
  - `Input`: 输入数据封装（l, m, reqs）
- **主要函数**:
  - `solve()`: 主调度算法（Bélády + 活跃锁定 + 部分卸载）
  - `merge_regions()`: 合并重叠内存区域
  - `find_missing_segments()`: 检测需要加载的缺失段
  - `find_next_use()`: Bélády算法核心 - 预测下次使用时间
  - `read_and_parse_input()`: 标准输入解析
  - `parse_input()`: 字符串输入解析（用于测试）
  - `main()`: 程序入口，支持三种模式

**算法特点**:
- ✅ Bélády最优页面替换
- ✅ 活跃请求锁定机制
- ✅ 部分卸载优化
- ✅ Just-in-Time加载
- ✅ 并行RW/Visit调度

#### `src/tests.rs` (150行)
**功能**: README示例测试框架
- 测试示例一: L=200, M=100, N=2 → Fin=12040
- 测试示例二: L=300, M=200, N=3 → Fin=12020
- 验证逻辑: 精确匹配或更优

#### `src/extended_tests.rs` (145行)
**功能**: 扩展测试用例集运行器
- 从 `test_cases.json` 加载测试数据
- 支持JSON反序列化（serde）
- 自动比对预期结果
- 生成详细测试报告

### 数据文件

#### `test_cases.json`
**格式**: JSON
**内容**: 10个综合测试用例
- 每个用例包含: id, name, description, input, expected_fin, explanation
- 覆盖场景: 内存复用、Bélády算法、部分重叠、并发访问等

### 文档文件

#### `README.md`
- 赛题完整描述
- 队伍信息
- 环境要求
- 输入输出格式
- 示例说明
- **新增**: 测试与验证章节、算法说明章节

#### `TEST_RESULTS.md`
- 所有测试用例的运行结果
- 性能统计与分析
- 算法优势与改进空间

#### `TEST_CASES_EXPLANATION.md`
- 每个测试用例的详细解析
- 场景特点说明
- 最优策略分析
- 难度分级

### 配置文件

#### `Cargo.toml`
```toml
[package]
name = "gmp_for_llm"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## 使用方式

### 编译
```bash
cargo build          # 开发版本
cargo build --release  # 优化版本
```

### 运行
```bash
# 模式1: 标准输入（手动输入测试数据）
cargo run

# 模式2: README示例测试
cargo run test

# 模式3: 扩展测试用例集
cargo run extended
```

### 测试
```bash
# 运行所有测试
cargo run test && cargo run extended

# 只看结果摘要
cargo run test 2>&1 | grep -E "示例|通过"
cargo run extended 2>&1 | grep -E "测试 #|通过率"
```

## 代码统计

| 文件 | 行数 | 说明 |
|------|------|------|
| `src/main.rs` | 307 | 核心算法 |
| `src/tests.rs` | 150 | README测试 |
| `src/extended_tests.rs` | 145 | 扩展测试 |
| **总计** | **602** | 纯代码行数 |

## 依赖关系

```
main.rs
  ├── tests.rs (mod tests)
  └── extended_tests.rs (mod extended_tests)
       └── test_cases.json (数据文件)
```

## 测试覆盖

- **基础测试**: 2个（README示例） - 100%通过
- **扩展测试**: 10个（综合场景） - 90%通过
- **总计**: 12个测试用例

## 性能特点

- **时间复杂度**: O(N² R log R)
  - N: 请求数量
  - R: HBM中的内存区域数
- **空间复杂度**: O(N + R)
- **实际表现**: 
  - 7/10 完美匹配
  - 2/10 超越预期
  - 1/10 待优化（密集并发场景）

## 开发环境

```
rustup 1.28.2
cargo 1.91.0
rustc 1.91.0
```

---

**最后更新**: 2025-11-03  
**版本**: 1.0
