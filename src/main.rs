use std::io::{self, Read};

#[derive(Debug)]
struct Req { addr: i64, size: i64, start: i64, time: i64 }

fn main() {
    // 读取全部 stdin
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut it = s.split_whitespace();
    let _l: i64 = it.next().unwrap().parse().unwrap();
    let m: i64 = it.next().unwrap().parse().unwrap();
    let n: usize = it.next().unwrap().parse().unwrap();
    let mut reqs: Vec<Req> = Vec::new();
    for _ in 0..n {
        let addr: i64 = it.next().unwrap().parse().unwrap();
        let size: i64 = it.next().unwrap().parse().unwrap();
        let start: i64 = it.next().unwrap().parse().unwrap();
        let time: i64 = it.next().unwrap().parse().unwrap();
        reqs.push(Req{addr,size,start,time});
    }

    // 优化策略：尽量让 Reload 和 Visit 并行
    // 读写操作（Reload/Offload）互相不能并行
    // 计算操作（Visit）互相不能并行
    // 读写操作可以与计算操作并行
    let mut last_rw_end: i64 = 0;      // 读写类上次结束时间
    let mut last_visit_end: i64 = 0;   // 计算类上次结束时间
    let mut hbm_occupied: Vec<(i64, i64, usize)> = Vec::new(); // (addr, size, req_idx)

    for (i, r) in reqs.iter().enumerate() {
        let load_time = 40 * r.size;
        
        // 计算理想的 Reload 开始时间：尽量让 Reload 在 Visit 前完成
        // 为了让 Visit 能在 r.start 时刻开始，Reload 需要在 r.start - load_time 时刻开始
        let ideal_reload_start = r.start - load_time;
        
        // 实际 Reload 开始时间：不能早于上一个读写操作结束
        let reload_start = std::cmp::max(ideal_reload_start, last_rw_end);
        
        println!("Reload {} {} {}", reload_start, r.addr, r.size);
        let reload_end = reload_start + load_time;
        last_rw_end = reload_end;
        hbm_occupied.push((r.addr, r.size, i));

        // Visit 开始时间：必须满足三个条件
        // 1. Reload 已完成 (>= reload_end)
        // 2. 不早于请求的 start 时间 (>= r.start)
        // 3. 计算通道空闲 (>= last_visit_end)
        let visit_start = std::cmp::max(std::cmp::max(reload_end, r.start), last_visit_end);
        println!("Visit {} {}", visit_start, i);
        let visit_end = visit_start + r.time;
        last_visit_end = visit_end;

        // Offload：在访问结束后，检查哪些内存不再需要并卸载
        // 简单策略：每次访问结束后立即卸载该块内存
        let off_start = std::cmp::max(visit_end, last_rw_end);
        
        // 检查是否需要提前卸载以腾出空间
        let mut current_hbm: i64 = hbm_occupied.iter().map(|(_, s, _)| s).sum();
        
        // 如果下一个请求需要空间且会超过 HBM 容量，提前卸载当前块
        let need_offload = if i + 1 < reqs.len() {
            current_hbm + reqs[i + 1].size > m
        } else {
            true // 最后一个请求，可以卸载
        };

        if need_offload {
            // 卸载当前访问的内存
            let off_time = 40 * r.size;
            println!("Offload {} {} {}", off_start, r.addr, r.size);
            let off_end = off_start + off_time;
            last_rw_end = off_end;
            hbm_occupied.retain(|(a, _, _)| *a != r.addr);
        }
    }

    let fin = std::cmp::max(last_rw_end, last_visit_end);
    println!("Fin {}", fin);
}
