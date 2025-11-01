use std::io::{self, BufRead};

mod tests;

#[derive(Debug, Clone)]
struct Req { addr: i64, size: i64, start: i64, time: i64 }

#[derive(Debug, Clone)]
struct Input {
    l: i64,
    m: i64,
    reqs: Vec<Req>,
}

fn parse_input(input: &str) -> Input {
    let mut it = input.split_whitespace();
    let l: i64 = it.next().unwrap().parse().unwrap();
    let m: i64 = it.next().unwrap().parse().unwrap();
    let n: usize = it.next().unwrap().parse().unwrap();
    let mut reqs: Vec<Req> = Vec::new();
    for _ in 0..n {
        let addr: i64 = it.next().unwrap().parse().unwrap();
        let size: i64 = it.next().unwrap().parse().unwrap();
        let start: i64 = it.next().unwrap().parse().unwrap();
        let time: i64 = it.next().unwrap().parse().unwrap();
        reqs.push(Req{addr, size, start, time});
    }
    Input { l, m, reqs }
}

fn read_input_interactive() -> String {
    let stdin = io::stdin();
    let mut lines = Vec::new();
    let mut first_line = String::new();
    
    // 读取第一行获取 N
    stdin.lock().read_line(&mut first_line).unwrap();
    lines.push(first_line.trim().to_string());
    
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    if parts.len() < 3 {
        panic!("Invalid input format");
    }
    let n: usize = parts[2].parse().unwrap();
    
    // 读取接下来的 N 行
    for _ in 0..n {
        let mut line = String::new();
        stdin.lock().read_line(&mut line).unwrap();
        if !line.trim().is_empty() {
            lines.push(line.trim().to_string());
        }
    }
    
    lines.join("\n")
}

fn solve(data: &Input) -> String {
    let mut output = Vec::new();

    // 优化策略：尽量让 Reload 和 Visit 并行
    let mut last_rw_end: i64 = 0;
    let mut last_visit_end: i64 = 0;
    let mut hbm_occupied: Vec<(i64, i64, usize)> = Vec::new();

    for (i, r) in data.reqs.iter().enumerate() {
        let load_time = 40 * r.size;
        let ideal_reload_start = r.start - load_time;
        let reload_start = std::cmp::max(ideal_reload_start, last_rw_end);
        
        output.push(format!("Reload {} {} {}", reload_start, r.addr, r.size));
        let reload_end = reload_start + load_time;
        last_rw_end = reload_end;
        hbm_occupied.push((r.addr, r.size, i));

        let visit_start = std::cmp::max(std::cmp::max(reload_end, r.start), last_visit_end);
        output.push(format!("Visit {} {}", visit_start, i));
        let visit_end = visit_start + r.time;
        last_visit_end = visit_end;

        let off_start = std::cmp::max(visit_end, last_rw_end);
        let current_hbm: i64 = hbm_occupied.iter().map(|(_, s, _)| s).sum();
        let need_offload = if i + 1 < data.reqs.len() {
            current_hbm + data.reqs[i + 1].size > data.m
        } else {
            true
        };

        if need_offload {
            let off_time = 40 * r.size;
            output.push(format!("Offload {} {} {}", off_start, r.addr, r.size));
            let off_end = off_start + off_time;
            last_rw_end = off_end;
            hbm_occupied.retain(|(a, _, _)| *a != r.addr);
        }
    }

    let fin = std::cmp::max(last_rw_end, last_visit_end);
    output.push(format!("Fin {}", fin));
    output.join("\n")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && args[1] == "test" {
        // 测试模式：需要包装 solve 函数以兼容测试接口
        tests::run_tests(|input| {
            let data = parse_input(input);
            solve(&data)
        });
    } else {
        // 正常模式：从标准输入读取
        let input_str = read_input_interactive();
        let data = parse_input(&input_str);
        let result = solve(&data);
        println!("{}", result);
    }
}
