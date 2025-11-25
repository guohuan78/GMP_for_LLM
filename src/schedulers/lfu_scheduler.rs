/// LFU 调度器 (Least Frequently Used)
/// 策略：基于访问频率淘汰。
/// 目的：保留热点数据（Weights/System Prompt），防止它们因为偶尔的间隔被 LRU 误杀。

use std::collections::{BTreeMap, HashMap};
use std::env;
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments, remove_region};

pub struct LfuScheduler;

impl LfuScheduler {
    pub fn new() -> Self {
        Self
    }
    
    // 辅助函数：查找 Next Use (作为 LFU 的 Tie-breaker)
    fn find_next_use(addr: i64, size: i64, current_idx: usize, reqs: &Vec<Req>) -> i64 {
        let region_end = addr + size;
        for i in (current_idx + 1)..reqs.len() {
            let r = &reqs[i];
            if std::cmp::max(addr, r.addr) < std::cmp::min(region_end, r.addr + r.size) {
                return r.start;
            }
        }
        i64::MAX
    }

    pub fn process_group(
        reqs: &Vec<Req>,
        group_indices: &Vec<usize>,
        hbm: &mut Vec<(i64, i64)>,
        freq_map: &mut HashMap<i64, u32>, // 地址 -> 访问次数
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        group_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        m: i64,
        current_req_idx: usize,
    ) {
        // 更新频率表
        for &idx in group_indices {
            let r = &reqs[idx];
            *freq_map.entry(r.addr).or_insert(0) += 1;
        }

        // --- 1. 标准加载 ---
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

        // --- 2. LFU Offload 策略 ---
        let load_sz: i64 = all_loads.iter().map(|(_, s)| s).sum();
        if load_sz > 0 {
            let cur_sz: i64 = hbm.iter().map(|(_, s)| s).sum();
            let mut need = (cur_sz + load_sz) - m;
            
            if need > 0 {
                let mut cand = Vec::new();
                for &(a, s) in hbm.iter() {
                    let freq = *freq_map.get(&a).unwrap_or(&0);
                    // Tie-breaker: 如果频率相同，使用 Belady (最远未来使用)
                    let nu = Self::find_next_use(a, s, current_req_idx, reqs);
                    cand.push((freq, nu, a, s));
                }
                
                // 排序：
                // 1. 频率升序 (小的先扔)
                // 2. NextUse 降序 (远的先扔)
                cand.sort_by(|(f1, nu1, _, _), (f2, nu2, _, _)| {
                    if f1 != f2 {
                        f1.cmp(f2) 
                    } else {
                        nu2.cmp(nu1)
                    }
                });
                
                let mut protect = Vec::new();
                for &idx in group_indices { protect.push((reqs[idx].addr, reqs[idx].size)); }
                merge_regions(&mut protect);

                for &(_, _, a, s) in &cand {
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

        // --- 3. Reload & Visit ---
        if !all_loads.is_empty() {
             let group_req_start = reqs[group_indices[0]].start;
             let total_reload = load_sz * 40;
             let reload_start = std::cmp::max(std::cmp::max(*last_rw_end, *last_visit_end), group_req_start - total_reload);
             let mut t = reload_start;
             for (la, lsz) in all_loads {
                 output.push(format!("Reload {} {} {}", t, la, lsz));
                 t += lsz * 40;
                 hbm.push((la, lsz));
                 // 新载入的，初始频率设为1
                 *freq_map.entry(la).or_insert(0) += 1;
             }
             *last_rw_end = t;
             merge_regions(hbm);
        }

        let visit_start = std::cmp::max(reqs[group_indices[0]].start, std::cmp::max(*last_rw_end, *last_visit_end));
        for &idx in group_indices {
            let r = &reqs[idx];
            output.push(format!("Visit {} {}", visit_start, r.id));
            *group_visit_end = std::cmp::max(*group_visit_end, visit_start + r.time);
            active_requests.entry(visit_start + r.time).or_default().push((r.addr, r.size));
        }
    }
}

impl Scheduler for LfuScheduler {
    fn name(&self) -> &str {
        "LFU Scheduler (Hotspot)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let debug = env::var("GMP_DEBUG").map(|v| v == "1").unwrap_or(false);

        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut freq_map: HashMap<i64, u32> = HashMap::new();
        
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

        let mut i = 0;
        while i < reqs.len() {
            let start = reqs[i].start;
            let mut j = i + 1;
            while j < reqs.len() && reqs[j].start == start { j += 1; }
            let group_indices: Vec<usize> = (i..j).collect();
            
            if debug {
                eprintln!("LFU_GROUP start={} count={}", start, group_indices.len());
            }

            active_requests = active_requests.split_off(&last_rw_end);
            let mut group_visit_end = last_visit_end;
            
            Self::process_group(
                reqs,
                &group_indices,
                &mut hbm,
                &mut freq_map,
                &mut output,
                &mut last_rw_end,
                &mut last_visit_end,
                &mut group_visit_end,
                &mut active_requests,
                m,
                group_indices[0],
            );
            
            last_visit_end = group_visit_end;
            i = j;
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}