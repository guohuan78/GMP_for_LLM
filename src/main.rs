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

fn read_and_parse_input() -> Input {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut first_line = String::new();
    
    handle.read_line(&mut first_line).unwrap();
    let parts: Vec<&str> = first_line.trim().split_whitespace().collect();
    if parts.len() < 3 {
        panic!("Invalid input format");
    }
    
    let l: i64 = parts[0].parse().unwrap();
    let m: i64 = parts[1].parse().unwrap();
    let n: usize = parts[2].parse().unwrap();
    
    let mut reqs: Vec<Req> = Vec::new();
    for _ in 0..n {
        let mut line = String::new();
        handle.read_line(&mut line).unwrap();
        let nums: Vec<&str> = line.trim().split_whitespace().collect();
        if nums.len() >= 4 {
            let addr: i64 = nums[0].parse().unwrap();
            let size: i64 = nums[1].parse().unwrap();
            let start: i64 = nums[2].parse().unwrap();
            let time: i64 = nums[3].parse().unwrap();
            reqs.push(Req{addr, size, start, time});
        }
    }
    
    Input { l, m, reqs }
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

fn solve(data: &Input) -> String {
    let mut output = Vec::new();
    let mut last_rw_end: i64 = 0;
    let mut last_visit_end: i64 = 0;
    
    // 跟踪 HBM 中的内存区间 (addr, size)
    let mut hbm_regions: Vec<(i64, i64)> = Vec::new();

    for (i, r) in data.reqs.iter().enumerate() {
        // 检查需要加载的部分
        let req_end = r.addr + r.size;
        let mut to_load: Vec<(i64, i64)> = Vec::new();
        
        // 将当前请求区间拆分为已覆盖和未覆盖部分
        let mut current_pos = r.addr;
        let mut regions_sorted = hbm_regions.clone();
        regions_sorted.sort_by_key(|(a, _)| *a);
        
        for (loaded_addr, loaded_size) in regions_sorted {
            let loaded_end = loaded_addr + loaded_size;
            
            // 检查是否与当前请求区间有重叠
            if loaded_end > r.addr && loaded_addr < req_end {
                let overlap_start = std::cmp::max(loaded_addr, r.addr);
                let overlap_end = std::cmp::min(loaded_end, req_end);
                
                // 如果有未覆盖的部分，需要加载
                if current_pos < overlap_start {
                    to_load.push((current_pos, overlap_start - current_pos));
                }
                
                current_pos = overlap_end;
            }
        }
        
        // 检查最后是否还有未覆盖部分
        if current_pos < req_end {
            to_load.push((current_pos, req_end - current_pos));
        }
        
        // Reload 需要加载的部分
        for (addr, size) in &to_load {
            let load_t = 40 * size;
            let ideal_start = r.start - load_t;
            let reload_start = std::cmp::max(ideal_start, last_rw_end);
            
            output.push(format!("Reload {} {} {}", reload_start, addr, size));
            last_rw_end = reload_start + load_t;
            
            // 添加到 HBM
            hbm_regions.push((*addr, *size));
        }
        
        // Visit
        let visit_start = std::cmp::max(std::cmp::max(last_rw_end, r.start), last_visit_end);
        output.push(format!("Visit {} {}", visit_start, i));
        let visit_end = visit_start + r.time;
        last_visit_end = visit_end;
        
        // Offload 策略：只在必要时卸载
        let is_last = i == data.reqs.len() - 1;
        
        if !is_last {
            let next_req = &data.reqs[i + 1];
            let next_req_end = next_req.addr + next_req.size;
            let current_hbm: i64 = hbm_regions.iter().map(|(_, s)| s).sum();
            
            // 计算下一个请求需要多少新空间
            let mut next_need = next_req.size;
            for (loaded_addr, loaded_size) in &hbm_regions {
                let loaded_end = loaded_addr + loaded_size;
                if loaded_end > next_req.addr && *loaded_addr < next_req_end {
                    let overlap_start = std::cmp::max(*loaded_addr, next_req.addr);
                    let overlap_end = std::cmp::min(loaded_end, next_req_end);
                    next_need -= overlap_end - overlap_start;
                }
            }
            
            if current_hbm + next_need > data.m {
                // 需要腾出空间
                let need_free = current_hbm + next_need - data.m;
                let mut freed: i64 = 0;
                let mut offload_list = Vec::new();
                let mut total_offload_time: i64 = 0;
                
                // 找出需要卸载的区域
                for (addr, size) in &hbm_regions {
                    if freed >= need_free {
                        break;
                    }
                    
                    let region_end = addr + size;
                    
                    // 检查这块内存是否与下一个请求重叠
                    if region_end <= next_req.addr || *addr >= next_req_end {
                        // 不重叠，可以完全卸载
                        let to_free = std::cmp::min(*size, need_free - freed);
                        offload_list.push((*addr, to_free));
                        freed += to_free;
                        total_offload_time += 40 * to_free;
                    } else {
                        // 重叠，只卸载不重叠的部分
                        let overlap_start = std::cmp::max(*addr, next_req.addr);
                        let overlap_end = std::cmp::min(region_end, next_req_end);
                        
                        // 左侧不重叠部分
                        if *addr < overlap_start {
                            let left_size = overlap_start - addr;
                            let to_free = std::cmp::min(left_size, need_free - freed);
                            offload_list.push((*addr, to_free));
                            freed += to_free;
                            total_offload_time += 40 * to_free;
                            if freed >= need_free {
                                continue;
                            }
                        }
                        
                        // 右侧不重叠部分
                        if overlap_end < region_end {
                            let right_size = region_end - overlap_end;
                            let to_free = std::cmp::min(right_size, need_free - freed);
                            offload_list.push((overlap_end, to_free));
                            freed += to_free;
                            total_offload_time += 40 * to_free;
                        }
                    }
                }
                
                // 计算两种策略的完成时间
                let next_load_time = 40 * next_need;
                
                // 策略1：Offload与Visit并行
                let parallel_rw_end = last_rw_end + total_offload_time;
                let parallel_reload_start = std::cmp::max(next_req.start - next_load_time, parallel_rw_end);
                let parallel_reload_end = parallel_reload_start + next_load_time;
                let parallel_visit_start = std::cmp::max(std::cmp::max(parallel_reload_end, next_req.start), visit_end);
                let parallel_visit_end = parallel_visit_start + next_req.time;
                let parallel_fin = std::cmp::max(parallel_reload_end, parallel_visit_end);
                
                // 策略2：Offload在Visit后
                let serial_off_start = std::cmp::max(visit_end, last_rw_end);
                let serial_rw_end = serial_off_start + total_offload_time;
                let serial_reload_start = std::cmp::max(next_req.start - next_load_time, serial_rw_end);
                let serial_reload_end = serial_reload_start + next_load_time;
                let serial_visit_start = std::cmp::max(std::cmp::max(serial_reload_end, next_req.start), visit_end);
                let serial_visit_end = serial_visit_start + next_req.time;
                let serial_fin = std::cmp::max(serial_reload_end, serial_visit_end);
                
                // 选择完成时间更早的策略
                let off_start = if parallel_fin <= serial_fin {
                    last_rw_end
                } else {
                    serial_off_start
                };
                
                // 执行 Offload
                for (addr, size) in &offload_list {
                    let off_time = 40 * size;
                    output.push(format!("Offload {} {} {}", off_start, addr, size));
                    last_rw_end = off_start + off_time;
                    
                    // 从 HBM 中移除
                    hbm_regions.retain(|(a, s)| !(*a == *addr && *s == *size));
                }
            }
        }
    }

    let fin = std::cmp::max(last_rw_end, last_visit_end);
    output.push(format!("Fin {}", fin));
    output.join("\n")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && args[1] == "test" {
        tests::run_tests(|input| {
            let data = parse_input(input);
            solve(&data)
        });
    } else {
        let data = read_and_parse_input();
        let result = solve(&data);
        println!("{}", result);
    }
}
