/// Greedy 调度器（基于 Bélády 算法的贪心策略）
/// 这是当前的主要实现

use std::collections::BTreeMap;
use std::env;
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments};

pub struct GreedyScheduler;

impl GreedyScheduler {
    pub fn new() -> Self {
        Self
    }
    
    /// 查找下一次使用该内存区域的时间（Bélády 算法核心）
    fn find_next_use(addr: i64, size: i64, current_idx: usize, reqs: &Vec<Req>) -> i64 {
        let region_end = addr + size;
        for i in (current_idx + 1)..reqs.len() {
            let r = &reqs[i];
            if std::cmp::max(addr, r.addr) < std::cmp::min(region_end, r.addr + r.size) {
                return r.start;
            }
        }
        i64::MAX / 4
    }
    
    /// 处理同时开始的请求组（组级别处理）
    fn process_simultaneous_group(
        reqs: &Vec<Req>,
        group_indices: &Vec<usize>,
        hbm: &mut Vec<(i64, i64)>,
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        group_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        m: i64,
        i: usize,
    ) {
        // 组级别处理：先收集并执行所有offload和reload，再统一visit
        let mut all_loads: Vec<(usize, Vec<(i64, i64)>)> = Vec::new();
        let mut temp_hbm = hbm.clone();

        // 第一遍：为每个请求计算需要load的数据
        for &req_idx in group_indices {
            let r = &reqs[req_idx];
            let to_load = find_missing_segments(&temp_hbm, r.addr, r.size);

            if !to_load.is_empty() {
                all_loads.push((req_idx, to_load.clone()));
                for &(la, lsz) in &to_load {
                    temp_hbm.push((la, lsz));
                }
                merge_regions(&mut temp_hbm);
            }
        }

        // 计算是否需要offload
        let total_load_size: i64 = all_loads.iter().flat_map(|(_, v)| v).map(|&(_, s)| s).sum();
        let mut to_offload: Vec<(i64, i64)> = Vec::new();

        if total_load_size > 0 {
            let cur_hbm_size: i64 = hbm.iter().map(|&(_, s)| s).sum();
            let mut need = (cur_hbm_size + total_load_size) - m;

            if need > 0 {
                let mut cand: Vec<(i64, i64, i64)> = Vec::new();
                for &(a, s) in hbm.iter() {
                    let nu = Self::find_next_use(a, s, i, reqs);
                    cand.push((nu, a, s));
                }
                cand.sort_by_key(|k| -k.0);

                // 收集组内所有请求的区域，避免offload这些区域
                let mut group_regions: Vec<(i64, i64)> = Vec::new();
                for &req_idx in group_indices {
                    let r = &reqs[req_idx];
                    group_regions.push((r.addr, r.size));
                }
                merge_regions(&mut group_regions);

                for &(_nu, a, s) in &cand {
                    if need <= 0 {
                        break;
                    }
                    let region_end = a + s;

                    // 检查是否与组内任何请求重叠
                    let mut overlaps_group = false;
                    for &(gr_addr, gr_size) in &group_regions {
                        if a < gr_addr + gr_size && region_end > gr_addr {
                            overlaps_group = true;
                            break;
                        }
                    }

                    if !overlaps_group {
                        let take = std::cmp::min(s, need);
                        to_offload.push((a, take));
                        need -= take;
                    }
                }
            }
        }

        // 执行offload
        if !to_offload.is_empty() {
            let mut start_t = *last_rw_end;
            for &(oa, osz) in &to_offload {
                for (&visit_end, locked) in active_requests.iter() {
                    for &(la, lsz) in locked {
                        if std::cmp::max(oa, la) < std::cmp::min(oa + osz, la + lsz) {
                            start_t = std::cmp::max(start_t, visit_end);
                        }
                    }
                }
            }
            let mut t = start_t;
            for &(oa, osz) in &to_offload {
                output.push(format!("Offload {} {} {}", t, oa, osz));
                t += osz * 40;
                hbm.retain(|&(ha, hs)| !(ha == oa && hs == osz));
            }
            *last_rw_end = t;
        }

        // 执行所有reload
        if !all_loads.is_empty() {
            let group_req_start = reqs[group_indices[0]].start;
            let total_reload = total_load_size * 40;
            let reload_start = std::cmp::max(*last_rw_end, group_req_start - total_reload);
            let mut t = reload_start;

            for (_, loads) in &all_loads {
                for &(la, lsz) in loads {
                    output.push(format!("Reload {} {} {}", t, la, lsz));
                    t += lsz * 40;
                    hbm.push((la, lsz));
                }
            }
            *last_rw_end = t;
            merge_regions(hbm);
        }

        // 统一的visit时间
        let group_req_start = reqs[group_indices[0]].start;
        let visit_start = std::cmp::max(group_req_start, std::cmp::max(*last_rw_end, *last_visit_end));

        // 同时发出所有visit
        for &req_idx in group_indices {
            let r = &reqs[req_idx];
            output.push(format!("Visit {} {}", visit_start, r.id));
            let visit_end = visit_start + r.time;
            *group_visit_end = std::cmp::max(*group_visit_end, visit_end);
            active_requests.entry(visit_end).or_default().push((r.addr, r.size));
        }
    }

    /// 处理单个请求（原始逻辑）
    fn process_single_requests(
        reqs: &Vec<Req>,
        group_indices: &Vec<usize>,
        hbm: &mut Vec<(i64, i64)>,
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        group_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        m: i64,
        debug: bool,
    ) {
        for &req_idx in group_indices {
            let r = &reqs[req_idx];

            let to_load = find_missing_segments(hbm, r.addr, r.size);
            let total_load: i64 = to_load.iter().map(|&(_, s)| s).sum();

            let mut to_offload: Vec<(i64, i64)> = Vec::new();
            if total_load > 0 {
                let cur_hbm_size: i64 = hbm.iter().map(|&(_, s)| s).sum();
                let mut need = (cur_hbm_size + total_load) - m;
                if need > 0 {
                    let mut cand: Vec<(i64, i64, i64)> = Vec::new();
                    for &(a, s) in hbm.iter() {
                        let nu = Self::find_next_use(a, s, req_idx, reqs);
                        cand.push((nu, a, s));
                    }
                    cand.sort_by_key(|k| -k.0);
                    for &(_nu, a, s) in &cand {
                        if need <= 0 {
                            break;
                        }
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
                                if take > 0 {
                                    to_offload.push((a, take));
                                    need -= take;
                                }
                            }
                            if need > 0 && overlap_end < region_end {
                                let right_sz = region_end - overlap_end;
                                let take = std::cmp::min(right_sz, need);
                                if take > 0 {
                                    to_offload.push((overlap_end, take));
                                    need -= take;
                                }
                            }
                        }
                    }
                }
            }

            if debug {
                eprintln!("REQ {} -> to_load={:?} to_offload={:?}", r.id, to_load, to_offload);
            }

            if !to_offload.is_empty() {
                let mut start_t = *last_rw_end;
                for &(oa, osz) in &to_offload {
                    for (&visit_end, locked) in active_requests.iter() {
                        for &(la, lsz) in locked {
                            if std::cmp::max(oa, la) < std::cmp::min(oa + osz, la + lsz) {
                                start_t = std::cmp::max(start_t, visit_end);
                            }
                        }
                    }
                }
                let mut t = start_t;
                for &(oa, osz) in &to_offload {
                    output.push(format!("Offload {} {} {}", t, oa, osz));
                    t += osz * 40;
                    hbm.retain(|&(ha, hs)| !(ha == oa && hs == osz));
                }
                *last_rw_end = t;
            }

            if !to_load.is_empty() {
                let total_reload = total_load * 40;
                let reload_start = std::cmp::max(*last_rw_end, r.start - total_reload);
                let mut t = reload_start;
                for &(la, lsz) in &to_load {
                    output.push(format!("Reload {} {} {}", t, la, lsz));
                    t += lsz * 40;
                    hbm.push((la, lsz));
                }
                *last_rw_end = t;
                merge_regions(hbm);
            }

            let visit_start = std::cmp::max(r.start, std::cmp::max(*last_rw_end, *last_visit_end));
            output.push(format!("Visit {} {}", visit_start, r.id));
            let visit_end = visit_start + r.time;
            *group_visit_end = std::cmp::max(*group_visit_end, visit_end);
            active_requests.entry(visit_end).or_default().push((r.addr, r.size));
        }
    }
}

impl Scheduler for GreedyScheduler {
    fn name(&self) -> &str {
        "GreedyScheduler (Bélády)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let debug = env::var("GMP_DEBUG").map(|v| v == "1").unwrap_or(false);
        
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

        let mut i = 0usize;
        while i < reqs.len() {
            let group_start = reqs[i].start;
            let mut j = i + 1;
            while j < reqs.len() && reqs[j].start == group_start {
                j += 1;
            }

            active_requests = active_requests.split_off(&last_rw_end);

            let mut group_indices: Vec<usize> = (i..j).collect();
            let group_rule = env::var("GMP_GROUP_RULE").unwrap_or_else(|_| "addr".to_string());
            match group_rule.as_str() {
                "time" => group_indices.sort_by_key(|&idx| -reqs[idx].time),
                "addr" => group_indices.sort_by_key(|&idx| -reqs[idx].addr),
                _ => (),
            }
            if debug {
                eprintln!("GROUP start={} idxs={:?} rule={}", group_start, group_indices, group_rule);
            }

            let mut group_visit_end = last_visit_end;

            // 判断是否为同一组（start时间相同且大于1个请求）
            let is_simultaneous_group = j - i > 1;

            if is_simultaneous_group {
                Self::process_simultaneous_group(
                    reqs,
                    &group_indices,
                    &mut hbm,
                    &mut output,
                    &mut last_rw_end,
                    &mut last_visit_end,
                    &mut group_visit_end,
                    &mut active_requests,
                    m,
                    i,
                );
            } else {
                Self::process_single_requests(
                    reqs,
                    &group_indices,
                    &mut hbm,
                    &mut output,
                    &mut last_rw_end,
                    &mut last_visit_end,
                    &mut group_visit_end,
                    &mut active_requests,
                    m,
                    debug,
                );
            }

            last_visit_end = group_visit_end;
            i = j;
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}
