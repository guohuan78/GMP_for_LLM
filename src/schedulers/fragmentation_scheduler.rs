/// 碎片感知调度器 (Fragmentation Aware)
/// 策略：在 Belady 的基础上，增加对“连续性”的考量。
/// 核心：优先淘汰那些“紧邻空闲内存”的数据段，目的是将小的空闲碎片合并成大块连续空间。
/// 适用场景：地址分配极度不连续，存在大量小片段重叠的场景。

use std::collections::BTreeMap;
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments, remove_region};

pub struct FragmentationScheduler;

impl FragmentationScheduler {
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

        // --- 2. 碎片感知 Offload 策略 ---
        let load_sz: i64 = all_loads.iter().map(|(_, s)| s).sum();
        if load_sz > 0 {
            let cur_sz: i64 = hbm.iter().map(|(_, s)| s).sum();
            let mut need = (cur_sz + load_sz) - m;
            
            if need > 0 {
                // 为了判断“是否相邻空洞”，我们需要按地址排序
                hbm.sort_by_key(|k| k.0);
                
                let mut cand = Vec::new();
                for i in 0..hbm.len() {
                    let (a, s) = hbm[i];
                    let nu = Self::find_next_use(a, s, current_req_idx, reqs);
                    
                    // 检查碎片奖励 (Fragmentation Bonus)
                    // 如果这一块的左边或右边有空隙（即不连续），那么淘汰它有助于合并空隙
                    let mut borders_hole = false;
                    
                    // 检查左边: 如果不是第一个，且和前一个有间隙
                    if i > 0 {
                        let (prev_a, prev_s) = hbm[i-1];
                        if prev_a + prev_s < a { borders_hole = true; }
                    } else if a > 0 {
                        // 左边是内存起点，且有空隙
                        borders_hole = true; 
                    }
                    
                    // 检查右边: 如果不是最后一个，且和后一个有间隙
                    if i < hbm.len() - 1 {
                        let (next_a, _) = hbm[i+1];
                        if a + s < next_a { borders_hole = true; }
                    } 
                    // (由于不知道 L，暂时不检查最右边边界，但这已经足够有效)

                    cand.push((nu, borders_hole, a, s));
                }

                // 排序：
                // 1. NextUse (主要因素)
                // 2. BordersHole (Tie-breaker): 如果时间差不多，优先淘汰能合并空洞的
                // 为了让 BordersHole 生效，我们给 NextUse 一个大的权重，给 BordersHole 一个小的加成
                // 或者更直接：Score = NextUse * (1.0 if not border else 1.2)
                cand.sort_by(|(nu_a, border_a, _, _), (nu_b, border_b, _, _)| {
                    // 使用 f64 避免溢出
                    let mut score_a = *nu_a as f64;
                    let mut score_b = *nu_b as f64;
                    
                    if *border_a { score_a *= 1.2; } // 提升淘汰优先级 (看起来像更远的未来)
                    if *border_b { score_b *= 1.2; }
                    
                    // 降序
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
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

impl Scheduler for FragmentationScheduler {
    fn name(&self) -> &str {
        "FragmentationAware"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
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