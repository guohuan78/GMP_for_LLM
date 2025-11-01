pub fn run_tests(solve_fn: fn(&str) -> String) {
    println!("=== 测试示例一 ===");
    let input1 = "200 100 2
0 100 0 30
100 100 50 10";
    
    let expected1 = "Reload 0 0 100
Visit 4000 0
Offload 4030 0 100
Reload 8030 100 100
Visit 12030 1
Fin 12040";
    
    let result1 = solve_fn(input1);
    println!("输入：");
    println!("{}", input1);
    println!("\n预期输出：");
    println!("{}", expected1);
    println!("\n实际输出：");
    println!("{}", result1);
    
    let result1_lines: Vec<&str> = result1.lines().collect();
    let expected1_lines: Vec<&str> = expected1.lines().collect();
    
    println!("\n分析：");
    if result1 == expected1 {
        println!("✓ 输出完全匹配！");
    } else {
        println!("✗ 输出不匹配");
        println!("  逐行对比：");
        for i in 0..std::cmp::max(result1_lines.len(), expected1_lines.len()) {
            let r = result1_lines.get(i).unwrap_or(&"<缺失>");
            let e = expected1_lines.get(i).unwrap_or(&"<缺失>");
            if r == e {
                println!("  ✓ 第{}行: {}", i+1, r);
            } else {
                println!("  ✗ 第{}行:", i+1);
                println!("    预期: {}", e);
                println!("    实际: {}", r);
            }
        }
    }
    
    // 提取 Fin 时间进行对比
    let fin1_expected = extract_fin_time(expected1);
    let fin1_actual = extract_fin_time(&result1);
    println!("\n完成时间对比：");
    println!("  预期: {}", fin1_expected);
    println!("  实际: {}", fin1_actual);
    if fin1_actual == fin1_expected {
        println!("  ✓ 完成时间相同");
    } else {
        let diff = fin1_actual - fin1_expected;
        println!("  ✗ 相差: {} (实际 {} 预期)", diff, if diff > 0 { "慢于" } else { "快于" });
    }
    
    println!("\n{}", "=".repeat(50));
    println!("\n=== 测试示例二 ===");
    let input2 = "300 200 3
0 100 0 50
100 100 4000 30
150 100 4001 20";
    
    let expected2 = "Reload 0 0 100
Visit 4000 0
Reload 4000 100 100
Visit 8000 1
Offload 8000 0 50
Reload 10000 200 50
Visit 12000 2
Fin 12020";
    
    let result2 = solve_fn(input2);
    println!("输入：");
    println!("{}", input2);
    println!("\n预期输出：");
    println!("{}", expected2);
    println!("\n实际输出：");
    println!("{}", result2);
    
    let result2_lines: Vec<&str> = result2.lines().collect();
    let expected2_lines: Vec<&str> = expected2.lines().collect();
    
    println!("\n分析：");
    if result2 == expected2 {
        println!("✓ 输出完全匹配！");
    } else {
        println!("✗ 输出不匹配");
        println!("  逐行对比：");
        for i in 0..std::cmp::max(result2_lines.len(), expected2_lines.len()) {
            let r = result2_lines.get(i).unwrap_or(&"<缺失>");
            let e = expected2_lines.get(i).unwrap_or(&"<缺失>");
            if r == e {
                println!("  ✓ 第{}行: {}", i+1, r);
            } else {
                println!("  ✗ 第{}行:", i+1);
                println!("    预期: {}", e);
                println!("    实际: {}", r);
            }
        }
    }
    
    let fin2_expected = extract_fin_time(expected2);
    let fin2_actual = extract_fin_time(&result2);
    println!("\n完成时间对比：");
    println!("  预期: {}", fin2_expected);
    println!("  实际: {}", fin2_actual);
    if fin2_actual == fin2_expected {
        println!("  ✓ 完成时间相同");
    } else {
        let diff = fin2_actual - fin2_expected;
        println!("  ✗ 相差: {} (实际 {} 预期)", diff, if diff > 0 { "慢于" } else { "快于" });
    }
    
    println!("\n{}", "=".repeat(50));
    println!("\n=== 总结 ===");
    let match1 = result1 == expected1;
    let match2 = result2 == expected2;
    
    if match1 {
        println!("示例一: ✓ 通过");
    } else {
        let diff1 = fin1_actual - fin1_expected;
        println!("示例一: ✗ 未通过 (完成时间差值: {:+})", diff1);
    }
    
    if match2 {
        println!("示例二: ✓ 通过");
    } else {
        let diff2 = fin2_actual - fin2_expected;
        println!("示例二: ✗ 未通过 (完成时间差值: {:+})", diff2);
    }

}

fn extract_fin_time(output: &str) -> i64 {
    for line in output.lines() {
        if line.starts_with("Fin ") {
            return line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        }
    }
    0
}
