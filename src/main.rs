use std::collections::BTreeMap;
use std::env;
use std::io::{self, Read};

#[derive(Debug, Clone)]
struct Req { addr: i64, size: i64, start: i64, time: i64, id: usize }

fn merge_regions(regions: &mut Vec<(i64, i64)>) {
    regions.sort_by_key(|r| r.0);
    let mut out = Vec::new();
    for (a, s) in regions.iter() {
        if let Some((la, ls)) = out.last_mut() {
            if *a <= *la + *ls {
                let new_end = std::cmp::max(*la + *ls, *a + *s);
                *ls = new_end - *la;
            } else {
                out.push((*a, *s));
            }
        } else {
            out.push((*a, *s));
        }
    }
    *regions = out;
}

fn find_missing_segments(hbm: &Vec<(i64,i64)>, addr: i64, size: i64) -> Vec<(i64,i64)> {
    let mut missing = Vec::new();
    let mut cur = addr;
    let end = addr + size;
    let mut regs = hbm.clone();
    regs.sort_by_key(|r| r.0);
    for &(ra, rs) in regs.iter() {
        if ra + rs <= cur { continue; }
        if ra > cur {
            let gap_end = std::cmp::min(ra, end);
            if gap_end > cur {
                missing.push((cur, gap_end - cur));
            }
        }
        cur = std::cmp::max(cur, ra + rs);
        if cur >= end { break; }
    }
    if cur < end {
        missing.push((cur, end - cur));
    }
    missing
}

fn find_next_use(addr: i64, size: i64, current_idx: usize, reqs: &Vec<Req>) -> i64 {
    let region_end = addr + size;
    for i in (current_idx+1)..reqs.len() {
        let r = &reqs[i];
        if std::cmp::max(addr, r.addr) < std::cmp::min(region_end, r.addr + r.size) {
            return r.start;
        }
    }
    i64::MAX / 4
}

fn solve(input: &str) -> String {
    let debug = env::var("GMP_DEBUG").map(|v| v=="1").unwrap_or(false);
    let mut it = input.split_whitespace();
    let _l: i64 = it.next().unwrap().parse().unwrap();
    let m: i64 = it.next().unwrap().parse().unwrap();
    let n: usize = it.next().unwrap().parse().unwrap();
    let mut reqs: Vec<Req> = Vec::new();
    for id in 0..n {
        let addr: i64 = it.next().unwrap().parse().unwrap();
        let size: i64 = it.next().unwrap().parse().unwrap();
        let start: i64 = it.next().unwrap().parse().unwrap();
        let time: i64 = it.next().unwrap().parse().unwrap();
        reqs.push(Req{addr,size,start,time,id});
    }

    let mut output: Vec<String> = Vec::new();
    let mut hbm: Vec<(i64,i64)> = Vec::new();
    let mut last_rw_end: i64 = 0;
    let mut last_visit_end: i64 = 0;
    let mut active_requests: BTreeMap<i64, Vec<(i64,i64)>> = BTreeMap::new();

    let mut i = 0usize;
    while i < reqs.len() {
        let group_start = reqs[i].start;
        let mut j = i+1;
        while j < reqs.len() && reqs[j].start == group_start { j += 1; }

        active_requests = active_requests.split_off(&last_rw_end);

        let mut group_indices: Vec<usize> = (i..j).collect();
        let group_rule = env::var("GMP_GROUP_RULE").unwrap_or_else(|_| "addr".to_string());
        match group_rule.as_str() {
            "time" => group_indices.sort_by_key(|&idx| -reqs[idx].time),
            "addr" => group_indices.sort_by_key(|&idx| -reqs[idx].addr),
            _ => (),
        }
        if debug { eprintln!("GROUP start={} idxs={:?} rule={}", group_start, group_indices, group_rule); }

        let mut group_visit_end = last_visit_end;
        for &req_idx in &group_indices {
            let r = &reqs[req_idx];

            let to_load = find_missing_segments(&hbm, r.addr, r.size);
            let total_load: i64 = to_load.iter().map(|&(_,s)| s).sum();

            let mut to_offload: Vec<(i64,i64)> = Vec::new();
            if total_load > 0 {
                let cur_hbm_size: i64 = hbm.iter().map(|&(_,s)| s).sum();
                let mut need = (cur_hbm_size + total_load) - m;
                if need > 0 {
                    let mut cand: Vec<(i64,i64,i64)> = Vec::new();
                    for &(a,s) in &hbm {
                        let nu = find_next_use(a,s, req_idx, &reqs);
                        cand.push((nu,a,s));
                    }
                    cand.sort_by_key(|k| -k.0);
                    for &(_nu,a,s) in &cand {
                        if need <= 0 { break; }
                        let r_end = r.addr + r.size;
                        let region_end = a + s;
                        let overlaps = a < r_end && region_end > r.addr;
                        if !overlaps {
                            let take = std::cmp::min(s, need);
                            to_offload.push((a, take));
                            need -= take;
                        } else {
                            let overlap_start = std::cmp::max(a, r.addr);
                            let overlap_end = std::cmp::min(region_end, r_end);
                            if a < overlap_start {
                                let left_sz = overlap_start - a;
                                let take = std::cmp::min(left_sz, need);
                                if take>0 { to_offload.push((a,take)); need -= take; }
                            }
                            if need>0 && overlap_end < region_end {
                                let right_sz = region_end - overlap_end;
                                let take = std::cmp::min(right_sz, need);
                                if take>0 { to_offload.push((overlap_end,take)); need -= take; }
                            }
                        }
                    }
                }
            }

            if debug { eprintln!("REQ {} -> to_load={:?} to_offload={:?}", r.id, to_load, to_offload); }

            if !to_offload.is_empty() {
                let mut start_t = last_rw_end;
                for &(oa, osz) in &to_offload {
                    for (&visit_end, locked) in &active_requests {
                        for &(la, lsz) in locked {
                            if std::cmp::max(oa, la) < std::cmp::min(oa+osz, la+lsz) {
                                start_t = std::cmp::max(start_t, visit_end);
                            }
                        }
                    }
                }
                let mut t = start_t;
                for &(oa, osz) in &to_offload {
                    output.push(format!("Offload {} {} {}", t, oa, osz));
                    t += osz * 40;
                    hbm.retain(|&(ha,hs)| !(ha==oa && hs==osz));
                }
                last_rw_end = t;
            }

            if !to_load.is_empty() {
                let total_reload = total_load * 40;
                let reload_start = std::cmp::max(last_rw_end, r.start - total_reload);
                let mut t = reload_start;
                for &(la, lsz) in &to_load {
                    output.push(format!("Reload {} {} {}", t, la, lsz));
                    t += lsz * 40;
                    hbm.push((la, lsz));
                }
                last_rw_end = t;
                merge_regions(&mut hbm);
            }

            let visit_start = std::cmp::max(r.start, std::cmp::max(last_rw_end, last_visit_end));
            output.push(format!("Visit {} {}", visit_start, r.id));
            let visit_end = visit_start + r.time;
            group_visit_end = std::cmp::max(group_visit_end, visit_end);
            active_requests.entry(visit_end).or_default().push((r.addr, r.size));
        }

        last_visit_end = group_visit_end;
        i = j;
    }

    output.push(format!("Fin {}", last_visit_end));
    output.join("\n")
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    if arg == "test" {
        let examples = vec![
            ("200 100 2\n0 100 0 30\n100 100 50 10\n",
             "Reload 0 0 100\nVisit 4000 0\nOffload 4030 0 100\nReload 8030 100 100\nVisit 12030 1\nFin 12040"),
            ("300 200 3\n0 100 0 50\n100 100 4000 30\n150 100 4001 20\n",
             "Reload 0 0 100\nVisit 4000 0\nReload 4000 100 100\nVisit 8000 1\nOffload 8000 0 50\nReload 10000 200 50\nVisit 12000 2\nFin 12020"),
            ("300 200 3\n0 100 0 5000\n100 100 0 9000\n50 100 4001 20\n",
             "Reload 0 100 100\nVisit 4000 1\nReload 4000 0 100\nVisit 8000 0\nVisit 13000 2\nFin 13020"),
        ];
        for (idx,(inp,exp)) in examples.iter().enumerate() {
            println!("=== 测试示例{} ===", idx+1);
            let actual = solve(inp);
            if actual.trim() == exp.trim() {
                println!("✓ 输出完全匹配！");
            } else {
                println!("✗ 输出不匹配");
                println!("预期:\n{}", exp);
                println!("实际:\n{}", actual);
            }
            let exp_fin = exp.lines().last().and_then(|l| l.strip_prefix("Fin ")).and_then(|s| s.parse::<i64>().ok()).unwrap_or(-1);
            let act_fin = actual.lines().last().and_then(|l| l.strip_prefix("Fin ")).and_then(|s| s.parse::<i64>().ok()).unwrap_or(-1);
            println!("完成时间: 预期={}, 实际={}\n", exp_fin, act_fin);
        }
        return;
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap();
    let out = solve(&buf);
    println!("{}", out);
}
