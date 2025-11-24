mod memory;
mod scheduler;

use std::io::{self, BufRead};
use scheduler::solve;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    
    if arg == "test" {
        run_tests();
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
    let examples = vec![
        (
            "200 100 2\n0 100 0 30\n100 100 50 10\n",
            "Reload 0 0 100\nVisit 4000 0\nOffload 4030 0 100\nReload 8030 100 100\nVisit 12030 1\nFin 12040"
        ),
        (
            "300 200 3\n0 100 0 50\n100 100 4000 30\n150 100 4001 20\n",
            "Reload 0 0 100\nVisit 4000 0\nReload 4000 100 100\nVisit 8000 1\nOffload 8000 0 50\nReload 10000 200 50\nVisit 12000 2\nFin 12020"
        ),
        (
            "300 200 3\n0 100 0 5000\n100 100 0 5000\n50 100 4001 20\n",
            "Reload 0 100 100\nReload 4000 0 100\nVisit 8000 1\nVisit 8000 0\nVisit 13000 2\nFin 13020"
        ),
    ];
    
    for (idx, (inp, exp)) in examples.iter().enumerate() {
        println!("=== 测试示例{} ===", idx + 1);
        let actual = solve(inp);
        
        if actual.trim() == exp.trim() {
            println!("✓ 输出完全匹配！");
        } else {
            println!("✗ 输出不匹配");
            println!("预期:\n{}", exp);
            println!("实际:\n{}", actual);
        }
        
        let exp_fin = exp.lines().last()
            .and_then(|l| l.strip_prefix("Fin "))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1);
        let act_fin = actual.lines().last()
            .and_then(|l| l.strip_prefix("Fin "))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1);
        
        println!("完成时间: 预期={}, 实际={}\n", exp_fin, act_fin);
    }
}
