mod memory;
mod types;
mod scheduler_trait;
mod schedulers;
mod dispatcher;

use std::io::{self, BufRead};
use dispatcher::solve;
fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    
    if arg == "test" {
        run_tests();
        return;
    }
    
    if arg == "test-json" {
        run_json_tests();
        return;
    }
    
    // 按行读取输入，读完 N 行后停止
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut lines = Vec::new();
    
    // 读取第一行
    let mut first_line = String::new();
    reader.read_line(&mut first_line).unwrap();
    let first_line = first_line.trim().to_string();
    
    // 解析 N
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let n: usize = parts[2].parse().unwrap();
    
    lines.push(first_line);
    
    // 读取 N 行请求
    for _ in 0..n {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        lines.push(line.trim().to_string());
    }
    
    let buf = lines.join("\n");
    let out = solve(&buf);
    println!("{}", out);
}

fn run_tests() {
    use std::process::Command;
    use std::path::Path;

    let examples = vec![
        (
            "200 100 2\n0 100 0 30\n100 100 50 10\n",
            12040,
        ),
        (
            "300 200 3\n0 100 0 50\n100 100 4000 30\n150 100 4001 20\n",
            12020,
        ),
        (
            "300 200 3\n0 100 0 5000\n100 100 0 5000\n50 100 4001 20\n",
            13020,
        ),
    ];
    
    let checker_path = Path::new("checker/checker");
    let has_checker = checker_path.exists();
    
    if !has_checker {
        println!("警告: checker/checker 不存在，将跳过 checker 验证\n");
    }
    
    for (idx, (inp, expected_fin)) in examples.iter().enumerate() {
        println!("=== 测试示例{} ===", idx + 1);
        let actual = solve(inp);
        
        let act_fin = actual.lines().last()
            .and_then(|l| l.strip_prefix("Fin "))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1);
        
        // Checker 验证
        let mut checker_ok = true;
        if has_checker {
            let tmp = std::env::temp_dir();
            let in_path = tmp.join(format!("gmp_test_in_{}.txt", idx));
            let out_path = tmp.join(format!("gmp_test_out_{}.txt", idx));
            let ans_path = tmp.join(format!("gmp_test_ans_{}.txt", idx));
            
            std::fs::write(&in_path, inp).expect("写入临时输入文件失败");
            std::fs::write(&out_path, actual.as_bytes()).expect("写入临时输出文件失败");
            std::fs::write(&ans_path, "dummy").expect("写入临时答案文件失败");
            
            let output = Command::new(checker_path)
                .arg(&in_path)
                .arg(&out_path)
                .arg(&ans_path)
                .output()
                .expect("无法执行 checker");
            
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                println!("Checker: {}", stdout.trim());
            } else {
                checker_ok = false;
                println!("✗ Checker 验证失败");
                println!("Checker stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("Checker stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        // Fin 时间验证
        let fin_ok = act_fin == *expected_fin;
        
        if checker_ok && fin_ok {
            println!("✓ 测试通过（checker OK 且 Fin={} 符合预期）\n", act_fin);
        } else {
            println!("✗ 测试失败");
            if !checker_ok {
                println!("  - Checker 验证未通过");
            }
            if !fin_ok {
                println!("  - Fin 不匹配: 预期={} 实际={}", expected_fin, act_fin);
            }
            println!("实际输出:\n{}\n", actual);
        }
    }
}

fn run_json_tests() {
    use std::process::Command;
    use std::path::Path;
    
    // 读取 test_cases.json
    let json_content = std::fs::read_to_string("test_cases.json")
        .expect("无法读取 test_cases.json");
    
    let json: serde_json::Value = serde_json::from_str(&json_content)
        .expect("无法解析 test_cases.json");
    
    let test_cases = json["test_cases"].as_array()
        .expect("test_cases 字段不是数组");
    
    let checker_path = Path::new("checker/checker");
    let has_checker = checker_path.exists();
    
    if !has_checker {
        println!("警告: checker/checker 不存在，将跳过 checker 验证\n");
    }
    
    let mut passed = 0;
    let mut failed = 0;
    
    for (idx, test_case) in test_cases.iter().enumerate() {
        let name = test_case["name"].as_str().unwrap_or("未命名");
        let input = test_case["input"].as_str().unwrap_or("");
        let expected_fin = test_case["expected_fin"].as_i64().unwrap_or(-1);
        
        println!("=== {} ===", name);
        let actual = solve(input);
        
        let act_fin = actual.lines().last()
            .and_then(|l| l.strip_prefix("Fin "))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1);
        
        // Checker 验证
        let mut checker_ok = true;
        if has_checker {
            let tmp = std::env::temp_dir();
            let in_path = tmp.join(format!("gmp_json_in_{}.txt", idx));
            let out_path = tmp.join(format!("gmp_json_out_{}.txt", idx));
            let ans_path = tmp.join(format!("gmp_json_ans_{}.txt", idx));
            
            std::fs::write(&in_path, input).expect("写入临时输入文件失败");
            std::fs::write(&out_path, actual.as_bytes()).expect("写入临时输出文件失败");
            std::fs::write(&ans_path, "dummy").expect("写入临时答案文件失败");
            
            let output = Command::new(checker_path)
                .arg(&in_path)
                .arg(&out_path)
                .arg(&ans_path)
                .output()
                .expect("无法执行 checker");
            
            checker_ok = output.status.success();
            if !checker_ok {
                println!("✗ Checker 验证失败");
                println!("Checker stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        // Fin 时间验证
        let fin_ok = act_fin == expected_fin;
        
        if checker_ok && fin_ok {
            println!("✓ 测试通过（Fin={}）", act_fin);
            passed += 1;
        } else {
            println!("✗ 测试失败");
            if !checker_ok {
                println!("  - Checker 验证未通过");
            }
            if !fin_ok {
                println!("  - Fin 不匹配: 预期={} 实际={}", expected_fin, act_fin);
            }
            failed += 1;
        }
        println!();
    }
    
    println!("========================================");
    println!("测试完成: 通过 {}/{}", passed, passed + failed);
    if failed == 0 {
        println!("🎉 所有测试通过！");
    } else {
        println!("⚠️  {} 个测试失败", failed);
    }
}