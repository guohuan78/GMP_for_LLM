/// 自适应调度器 (Adaptive)
/// 策略：根据当前 HBM 的内存占用率（Pressure），动态在“激进预取”、“标准贪心”和“保守清理”之间切换。
/// 适用场景：负载波动大、阶段性明显的复杂混合任务。

use std::collections::BTreeMap;
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments, remove_region};

pub struct AdaptiveScheduler;

impl AdaptiveScheduler {
    pub fn new() -> Self {
        Self
    }

    fn find_next_use(addr: i64, size: i64, current_idx: usize, reqs: &Vec<Req>) -> i64 {
        let region_end = addr + size;
        for i in (current_idx + 1)..reqs.len() {
            let r = &reqs[i];
            if std::cmp::max(addr, r.addr) < std::cmp::min(region_end, r.addr + r.size) {
                return r.start;
            }
        }
        i64::MAX / 2
    }
    
    // 辅助：检查下一组是否需要（用于高压清理）
    fn is_needed_by_next_group(addr: i64, size: i64, next_indices: &Vec<usize>, reqs: &Vec<Req>) -> bool {
        let end = addr + size;
        for &idx in next_indices {
            let r = &reqs[idx];
            let r_end = r.addr + r.size;
            if std::cmp::max(addr, r.addr) < std::cmp::min(end, r_end) {
                return true;
            }
        }
        false
    }

    pub fn process_group(
        reqs: &Vec<Req>,
        group_indices: &Vec<usize>,
        next_group_indices: Option<&Vec<usize>>,
        hbm: &mut Vec<(i64, i64)>,
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        group_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        m: i64,
        current_req_idx: usize,
    ) {
        // 计算当前内存压力
        let cur_sz: i64 = hbm.iter().map(|(_, s)| s).sum();
        let pressure = cur_sz as f64 / m as f64;
        
        // --- 1. 标准加载流程 ---
        let mut all_loads = Vec::new();
        let mut temp_hbm = hbm.clone();
        for &idx in group_indices {
            let r = &reqs[idx];
            let missing = find_missing_segments(&temp_hbm, r.addr, r.size);
            if !missing.is_empty() {
                all_loads.extend(missing.clone());
                for (ma, ms) in missing { temp_hbm.push((ma, ms)); }
                merge_regions(&mut temp_hbm);
            }
        }

        // Offload Check (Always Belady)
        let load_sz: i64 = all_loads.iter().map(|(_, s)| s).sum();
        if load_sz > 0 {
            let cur_sz_inner: i64 = hbm.iter().map(|(_, s)| s).sum();
            let mut need = (cur_sz_inner + load_sz) - m;
            
            if need > 0 {
                let mut cand = Vec::new();
                for &(a, s) in hbm.iter() {
                    let nu = Self::find_next_use(a, s, current_req_idx, reqs);
                    cand.push((nu, a, s));
                }
                cand.sort_by_key(|k| -k.0);
                
                let mut protect = Vec::new();
                for &idx in group_indices { protect.push((reqs[idx].addr, reqs[idx].size)); }
                merge_regions(&mut protect);

                for &(_, a, s) in &cand {
                    if need <= 0 { break; }
                    let ae = a + s;
                    let mut overlap = false;
                    for &(pa, ps) in &protect {
                        if a < pa + ps && ae > pa { overlap = true; break; }
                    }
                    if !overlap {
                        let take = std::cmp::min(s, need);
                        let mut start_t = std::cmp::max(*last_rw_end, *last_visit_end);
                        for (&ve, locks) in active_requests.iter() {
                            for &(la, ls) in locks {
                                if std::cmp::max(a, la) < std::cmp::min(ae, la+ls) {
                                    start_t = std::cmp::max(start_t, ve);
                                }
                            }
                        }
                        output.push(format!("Offload {} {} {}", start_t, a, take));
                        *last_rw_end = start_t + take * 40;
                        remove_region(hbm, a, take);
                        need -= take;
                    }
                }
            }
        }

        // Reload
        if !all_loads.is_empty() {
             let group_req_start = reqs[group_indices[0]].start;
             let total_reload = load_sz * 40;
             let reload_start = std::cmp::max(std::cmp::max(*last_rw_end, *last_visit_end), group_req_start - total_reload);
             let mut t = reload_start;
             for (la, lsz) in all_loads {
                 output.push(format!("Reload {} {} {}", t, la, lsz));
                 t += lsz * 40;
                 hbm.push((la, lsz));
             }
             *last_rw_end = t;
             merge_regions(hbm);
        }

        // Visit
        let visit_start = std::cmp::max(reqs[group_indices[0]].start, std::cmp::max(*last_rw_end, *last_visit_end));
        for &idx in group_indices {
            let r = &reqs[idx];
            output.push(format!("Visit {} {}", visit_start, r.id));
            *group_visit_end = std::cmp::max(*group_visit_end, visit_start + r.time);
            active_requests.entry(visit_start + r.time).or_default().push((r.addr, r.size));
        }

        // --- 自适应分支策略 ---
        
        // 策略 A: 高压 (>85%) -> 触发 Lazy Cleanup
        // 目的：防止内存满载导致死锁，腾出空间
        if pressure > 0.85 {
            if let Some(n_idxs) = next_group_indices {
                let mut to_clean = Vec::new();
                for &(a, s) in hbm.iter() {
                    // 如果下一组不需要，且当前不被锁定（简化判断，只看是否 active，实际 remove_region 会处理）
                    if !Self::is_needed_by_next_group(a, s, n_idxs, reqs) {
                        to_clean.push((a, s));
                    }
                }
                
                let clean_start = std::cmp::max(*last_rw_end, *group_visit_end);
                let mut ct = clean_start;
                for (ca, cs) in to_clean {
                    // 仅当 HBM 仍拥挤时才清理
                    let current_total: i64 = hbm.iter().map(|(_, s)| s).sum();
                    if (current_total as f64 / m as f64) < 0.7 { break; } 
                    
                    output.push(format!("Offload {} {} {}", ct, ca, cs));
                    ct += cs * 40;
                    remove_region(hbm, ca, cs);
                }
                *last_rw_end = ct;
            }
        }
        // 策略 B: 低压 (<60%) -> 触发 Aggressive Prefetch
        // 目的：利用空闲带宽，填满内存
        else if pressure < 0.60 {
             if let Some(next_idxs) = next_group_indices {
                let prefetch_start = *last_rw_end;
                if prefetch_start < *group_visit_end {
                    let mut temp_hbm_prefetch = hbm.clone();
                    let mut prefetch_candidates = Vec::new();
                    
                    for &n_idx in next_idxs {
                        let nr = &reqs[n_idx];
                        let missing = find_missing_segments(&temp_hbm_prefetch, nr.addr, nr.size);
                        for (ma, ms) in missing {
                            prefetch_candidates.push((ma, ms));
                            temp_hbm_prefetch.push((ma, ms));
                            merge_regions(&mut temp_hbm_prefetch);
                        }
                    }

                    let mut cur_hbm_size: i64 = hbm.iter().map(|&(_, s)| s).sum();
                    let mut pt = prefetch_start;
                    for (pa, ps) in prefetch_candidates {
                        // 预取限制：不能超过 90% 容量
                        if (cur_hbm_size + ps) as f64 <= m as f64 * 0.9 {
                            output.push(format!("Reload {} {} {}", pt, pa, ps));
                            pt += ps * 40;
                            hbm.push((pa, ps));
                            cur_hbm_size += ps;
                        } else {
                            break;
                        }
                    }
                    merge_regions(hbm);
                    *last_rw_end = pt;
                }
            }
        }
    }
}

impl Scheduler for AdaptiveScheduler {
    fn name(&self) -> &str {
        "AdaptiveScheduler (Hybrid)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

        let mut groups = Vec::new();
        let mut i = 0;
        while i < reqs.len() {
            let start = reqs[i].start;
            let mut j = i + 1;
            while j < reqs.len() && reqs[j].start == start { j += 1; }
            groups.push((i..j).collect::<Vec<usize>>());
            i = j;
        }

        for (g_idx, group_indices) in groups.iter().enumerate() {
            let next_group_indices = if g_idx + 1 < groups.len() {
                Some(&groups[g_idx + 1])
            } else {
                None
            };
            
            active_requests = active_requests.split_off(&last_rw_end);
            let mut group_visit_end = last_visit_end;
            
            Self::process_group(
                reqs,
                group_indices,
                next_group_indices,
                &mut hbm,
                &mut output,
                &mut last_rw_end,
                &mut last_visit_end,
                &mut group_visit_end,
                &mut active_requests,
                m,
                group_indices[0],
            );
            
            last_visit_end = group_visit_end;
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}