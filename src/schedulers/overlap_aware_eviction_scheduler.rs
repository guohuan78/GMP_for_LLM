/// OverlapAwareEvictionScheduler - 重叠感知驱逐调度器
/// 
/// 核心思想：
/// 1. 精确处理部分重叠区域，避免重复 IO
/// 2. 卸载时保护“未来可能扩展访问”的边界（通过 look-ahead）
/// 3. 同组请求联合规划内存布局
use std::collections::BTreeMap;
use std::env;

use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments, remove_region};

pub struct OverlapAwareEvictionScheduler {
    lookahead: usize,
}

impl OverlapAwareEvictionScheduler {
    pub fn new() -> Self {
        let lookahead = env::var("GMP_LOOKAHEAD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50);
        Self { lookahead }
    }

    /// 查找未来是否会在 addr 附近扩展访问（用于保护边界）
    fn will_extend_near(&self, addr: i64, size: i64, current_idx: usize, reqs: &Vec<Req>) -> bool {
        let region_start = addr;
        let region_end = addr + size;
        let end = std::cmp::min(reqs.len(), current_idx + self.lookahead);
        for i in (current_idx + 1)..end {
            let r = &reqs[i];
            let r_end = r.addr + r.size;
            // 检查是否与当前区域相邻或重叠扩展
            if (r.addr < region_end && r_end > region_start) || // 重叠
               (r.addr == region_end) ||                         // 右邻接
               (r_end == region_start) {                         // 左邻接
                return true;
            }
        }
        false
    }
}

impl Scheduler for OverlapAwareEvictionScheduler {
    fn name(&self) -> &str {
        "OverlapAwareEvictionScheduler"
    }

    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        // let debug = env::var("GMP_DEBUG").map(|v| v == "1").unwrap_or(false);
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new(); // 当前 HBM 中的内存段
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

        let mut i = 0;
        while i < reqs.len() {
            let group_start_time = reqs[i].start;
            let mut j = i + 1;
            while j < reqs.len() && reqs[j].start == group_start_time {
                j += 1;
            }
            active_requests = active_requests.split_off(&last_rw_end);

            let group_indices: Vec<usize> = (i..j).collect();
            // let is_group = group_indices.len() > 1;

            // Step 1: 计算组内总内存需求（合并后）
            let mut group_union: Vec<(i64, i64)> = group_indices.iter().map(|&idx| (reqs[idx].addr, reqs[idx].size)).collect();
            merge_regions(&mut group_union);
            let total_group_size: i64 = group_union.iter().map(|&(_, s)| s).sum();

            if total_group_size > m {
                return Err(format!(
                    "Group starting at time {} requires {} memory, exceeds M={}",
                    group_start_time, total_group_size, m
                ));
            }

            // Step 2: 找出缺失段（相对于当前 HBM）
            let mut missing_all: Vec<(i64, i64)> = Vec::new();
            let mut temp_hbm = hbm.clone();
            for &(a, s) in &group_union {
                let missing = find_missing_segments(&temp_hbm, a, s);
                for seg in &missing {
                    temp_hbm.push(*seg);
                }
                merge_regions(&mut temp_hbm);
                missing_all.extend(missing);
            }
            merge_regions(&mut missing_all);
            let total_load = missing_all.iter().map(|&(_, s)| s).sum::<i64>();

            // Step 3: 若需腾空间，智能卸载
            if total_load > 0 {
                let current_usage: i64 = hbm.iter().map(|&(_, s)| s).sum();
                let mut need = current_usage + total_load - m;
                let mut to_offload: Vec<(i64, i64)> = Vec::new();

                if need > 0 {
                    // 候选：非活跃、非组内、且未来不扩展的区域
                    for &(ha, hs) in hbm.iter() {
                        if need <= 0 { break; }

                        // 检查是否与组内重叠 → 不能卸
                        let mut overlaps_group = false;
                        for &(ga, gs) in &group_union {
                            if ha < ga + gs && ha + hs > ga {
                                overlaps_group = true;
                                break;
                            }
                        }
                        if overlaps_group { continue; }

                        // 检查是否活跃 → 不能卸
                        let mut is_active = false;
                        for regions in active_requests.values() {
                            for &(aa, asz) in regions {
                                if ha < aa + asz && ha + hs > aa {
                                    is_active = true;
                                    break;
                                }
                            }
                            if is_active { break; }
                        }
                        if is_active { continue; }

                        // 检查未来是否会扩展访问此区域 → 尽量保留
                        if self.will_extend_near(ha, hs, i, reqs) {
                            continue; // 保护边界，跳过卸载
                        }

                        let take = std::cmp::min(hs, need);
                        to_offload.push((ha, take));
                        need -= take;
                    }

                    if need > 0 {
                        return Err("Cannot free enough space even after eviction".to_string());
                    }

                    // 执行卸载
                    let mut start_t = std::cmp::max(last_rw_end, last_visit_end);
                    for &(oa, osz) in &to_offload {
                        for (&ve, locked) in active_requests.iter() {
                            for &(la, ls) in locked {
                                if std::cmp::max(oa, la) < std::cmp::min(oa + osz, la + ls) {
                                    start_t = std::cmp::max(start_t, ve);
                                }
                            }
                        }
                    }
                    let mut t = start_t;
                    for &(oa, osz) in &to_offload {
                        output.push(format!("Offload {} {} {}", t, oa, osz));
                        t += osz * 40;
                        remove_region(&mut hbm, oa, osz);
                    }
                    last_rw_end = t;
                }

                // Step 4: 加载缺失段
                let earliest_reload = group_start_time.saturating_sub(total_load * 40);
                let reload_start = std::cmp::max(last_rw_end, std::cmp::max(last_visit_end, earliest_reload));
                let mut t = reload_start;
                for &(a, s) in &missing_all {
                    output.push(format!("Reload {} {} {}", t, a, s));
                    t += s * 40;
                    hbm.push((a, s));
                }
                last_rw_end = t;
                merge_regions(&mut hbm);
            }

            // Step 5: 执行 Visit（组内同时）
            let visit_start = std::cmp::max(group_start_time, std::cmp::max(last_rw_end, last_visit_end));
            let mut group_end = visit_start;
            for &idx in &group_indices {
                let r = &reqs[idx];
                output.push(format!("Visit {} {}", visit_start, r.id));
                group_end = std::cmp::max(group_end, visit_start + r.time);
                active_requests.entry(visit_start + r.time).or_default().push((r.addr, r.size));
            }
            last_visit_end = group_end;

            i = j;
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}