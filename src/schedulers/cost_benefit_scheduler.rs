/// 成本收益调度器 (Cost-Benefit)
/// 策略：综合考虑“时间紧迫性”和“重载成本(大小)”。
/// 公式：EvictionScore = (NextUse - Now) / RegionSize
/// 目的：优先保留那些“又大又急”的数据，优先淘汰“又小又远”的数据。

use std::collections::BTreeMap;
use std::env;
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments, remove_region};

pub struct CostBenefitScheduler;

impl CostBenefitScheduler {
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

    pub fn process_group(
        reqs: &Vec<Req>,
        group_indices: &Vec<usize>,
        hbm: &mut Vec<(i64, i64)>,
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        group_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        m: i64,
        current_req_idx: usize,
    ) {
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

        // --- 2. Cost-Benefit Offload 策略 ---
        let load_sz: i64 = all_loads.iter().map(|(_, s)| s).sum();
        if load_sz > 0 {
            let cur_sz: i64 = hbm.iter().map(|(_, s)| s).sum();
            let mut need = (cur_sz + load_sz) - m;
            
            if need > 0 {
                let mut cand = Vec::new();
                let current_time = std::cmp::max(*last_rw_end, reqs[current_req_idx].start);

                for &(a, s) in hbm.iter() {
                    let nu = Self::find_next_use(a, s, current_req_idx, reqs);
                    cand.push((nu, a, s));
                }

                // 排序逻辑：Score = TimeDiff / Size
                // 使用 f64 避免 u128 乘法溢出
                cand.sort_by(|(nu_a, _, s_a), (nu_b, _, s_b)| {
                    let time_diff_a = nu_a.saturating_sub(current_time) as f64;
                    let time_diff_b = nu_b.saturating_sub(current_time) as f64;
                    
                    let size_a = (*s_a).max(1) as f64;
                    let size_b = (*s_b).max(1) as f64;

                    // 分数越高 -> 越不重要（时间远 或 体积小） -> 优先淘汰
                    let score_a = time_diff_a / size_a;
                    let score_b = time_diff_b / size_b;

                    // 降序排列
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                });
                
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

impl Scheduler for CostBenefitScheduler {
    fn name(&self) -> &str {
        "CostBenefit (SizeAware)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        // 仅在设置环境变量 GMP_DEBUG=1 时启用调试日志
        let debug = env::var("GMP_DEBUG").map(|v| v == "1").unwrap_or(false);
        
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
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
                eprintln!("CB_GROUP start={} count={}", start, group_indices.len());
            }

            active_requests = active_requests.split_off(&last_rw_end);
            let mut group_visit_end = last_visit_end;
            
            Self::process_group(
                reqs,
                &group_indices,
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
            i = j;
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}