use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
struct TestRequest {
    addr: i64,
    size: i64,
    start: i64,
    time: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct TestInput {
    #[serde(rename = "L")]
    l: i64,
    #[serde(rename = "M")]
    m: i64,
    #[serde(rename = "N")]
    n: usize,
    requests: Vec<TestRequest>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TestCase {
    id: usize,
    name: String,
    description: String,
    input: TestInput,
    expected_output: Vec<String>,
    expected_fin: i64,
    explanation: String,
}

#[derive(Debug, Deserialize)]
struct TestSuite {
    test_cases: Vec<TestCase>,
}

pub fn load_test_cases() -> Result<Vec<TestCase>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("test_cases.json")?;
    let suite: TestSuite = serde_json::from_str(&content)?;
    Ok(suite.test_cases)
}

pub fn convert_test_to_input_string(test: &TestCase) -> String {
    let mut lines = vec![
        format!("{} {} {}", test.input.l, test.input.m, test.input.n)
    ];
    
    for req in &test.input.requests {
        lines.push(format!("{} {} {} {}", req.addr, req.size, req.start, req.time));
    }
    
    lines.join("\n")
}

pub fn run_extended_tests<F>(solver: F) 
where
    F: Fn(&str) -> String,
{
    println!("\n========================================");
    println!("      扩展测试用例集运行结果");
    println!("========================================\n");

    let test_cases = match load_test_cases() {
        Ok(cases) => cases,
        Err(e) => {
            println!("❌ 无法加载测试用例: {}", e);
            println!("   请确保 test_cases.json 文件存在于项目根目录");
            return;
        }
    };

    let mut passed = 0;
    let mut failed = 0;

    for test in test_cases {
        println!("┌─ 测试 #{}: {}", test.id, test.name);
        println!("│  描述: {}", test.description);
        
        let input_str = convert_test_to_input_string(&test);
        let output = solver(&input_str);
        
        let actual_lines: Vec<&str> = output.trim().split('\n').collect();
        
        // 提取实际的Fin时间
        let actual_fin: i64 = actual_lines
            .last()
            .and_then(|line| {
                if line.starts_with("Fin ") {
                    line.split_whitespace().nth(1)?.parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);

        // 验证输出
        let is_correct = actual_fin == test.expected_fin;
        let is_better = actual_fin < test.expected_fin;
        let is_acceptable = actual_fin <= test.expected_fin;

        if is_correct {
            passed += 1;
            println!("│  ✓ 完美匹配！");
            println!("│  预期 Fin: {}", test.expected_fin);
            println!("│  实际 Fin: {}", actual_fin);
        } else if is_better {
            passed += 1;
            println!("│  ✓✓ 超越最优！");
            println!("│  预期 Fin: {}", test.expected_fin);
            println!("│  实际 Fin: {} (提升 {})", actual_fin, test.expected_fin - actual_fin);
        } else if is_acceptable {
            passed += 1;
            println!("│  ✓ 通过（较慢但可接受）");
            println!("│  预期 Fin: {}", test.expected_fin);
            println!("│  实际 Fin: {} (慢 {})", actual_fin, actual_fin - test.expected_fin);
        } else {
            failed += 1;
            println!("│  ✗ 未通过");
            println!("│  预期 Fin: {}", test.expected_fin);
            println!("│  实际 Fin: {} (慢 {})", actual_fin, actual_fin - test.expected_fin);
            println!("│  说明: {}", test.explanation);
        }
        
        println!("└─────────────────────────────────────\n");
    }

    println!("========================================");
    println!("  总计: {} 个测试", passed + failed);
    println!("  通过: {} ✓", passed);
    println!("  失败: {} ✗", failed);
    println!("  通过率: {:.1}%", (passed as f64 / (passed + failed) as f64) * 100.0);
    println!("========================================\n");
}
