/// 分代调度器 (Generational / SLRU)
/// 策略：将内存划分为“考察区 (Probation)”和“保护区 (Protected)”。
/// 只有被访问至少两次的数据才能进入保护区。淘汰时优先淘汰考察区数据。
/// 目的：防止“一次性扫描 (Scan)”数据污染热点缓存。
/// 适用场景：存在大量一次性数据流，但也包含少量高频热点数据的混合负载。

use std::collections::{BTreeMap, HashSet};
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments, remove_region};

pub struct GenerationalScheduler;

impl GenerationalScheduler {
    pub fn new() -> Self {
        Self
    }
    
    // Belady Next Use 作为保底策略
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
        protected_set: &mut HashSet<i64>, // 记录哪些地址在保护区 (简化：只记录 start addr)
        hit_count: &mut BTreeMap<i64, u32>, // 记录击中次数
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        group_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        m: i64,
        current_req_idx: usize,
    ) {
        // --- 更新分代状态 (Promotion) ---
        for &idx in group_indices {
            let r = &reqs[idx];
            let count = hit_count.entry(r.addr).or_insert(0);
            *count += 1;
            
            // 如果被访问了 2 次及以上，晋升为 Protected
            if *count >= 2 {
                protected_set.insert(r.addr);
            }
        }

        // --- 1. Load ---
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

        // --- 2. Offload (Generational Policy) ---
        let load_sz: i64 = all_loads.iter().map(|(_, s)| s).sum();
        if load_sz > 0 {
            let cur_sz: i64 = hbm.iter().map(|(_, s)| s).sum();
            let mut need = (cur_sz + load_sz) - m;
            
            if need > 0 {
                let mut cand = Vec::new();
                for &(a, s) in hbm.iter() {
                    let nu = Self::find_next_use(a, s, current_req_idx, reqs);
                    // 判断是否在保护区
                    // 简化判断：如果该块的起始地址在 set 里，就算保护。
                    // (实际生产中应判断重叠，但这里足够 heuristic)
                    let is_protected = protected_set.contains(&a);
                    
                    cand.push((is_protected, nu, a, s));
                }

                // 排序逻辑：
                // 1. is_protected (升序): false (0) 排前面 -> 优先淘汰非保护区
                // 2. NextUse (降序): 远的排前面
                cand.sort_by(|(prot_a, nu_a, _, _), (prot_b, nu_b, _, _)| {
                    if prot_a != prot_b {
                        // false < true. 我们希望 false 排前面 (被淘汰)
                        // 所以用 a.cmp(b)
                        prot_a.cmp(prot_b)
                    } else {
                        // 同代竞争，Belady 规则
                        nu_b.cmp(nu_a)
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
                        
                        // 移除后，可以考虑从 protected_set 移除，也可以保留记忆
                        // 这里选择移除，意味着下次加载进来重新从 probation 开始 (SLRU 变种)
                        if take == s {
                            protected_set.remove(&a);
                            hit_count.remove(&a);
                        }
                    }
                }
            }
        }

        // --- 3. Reload ---
        if !all_loads.is_empty() {
             let group_req_start = reqs[group_indices[0]].start;
             let total_reload = load_sz * 40;
             let reload_start = std::cmp::max(std::cmp::max(*last_rw_end, *last_visit_end), group_req_start - total_reload);
             let mut t = reload_start;
             for (la, lsz) in all_loads {
                 output.push(format!("Reload {} {} {}", t, la, lsz));
                 t += lsz * 40;
                 hbm.push((la, lsz));
                 // 新加载的初始化计数为 0 或 1 (process_group 开头会加1)
                 // 但这里 reload 发生在开头逻辑之后吗？
                 // 注意：process_group 开头的 Promotion 是针对“本组需要访问的数据”。
                 // Reload 是为了满足这些访问。所以逻辑上是自洽的。
                 // 只是对于 Reload 进来的新块，如果它不在本组访问列表里（预取情况），那它就是 0。
                 // 但本函数只加载本组需要的，所以肯定会被 Visit 覆盖到。
             }
             *last_rw_end = t;
             merge_regions(hbm);
        }

        // 4. Visit
        let visit_start = std::cmp::max(reqs[group_indices[0]].start, std::cmp::max(*last_rw_end, *last_visit_end));
        for &idx in group_indices {
            let r = &reqs[idx];
            output.push(format!("Visit {} {}", visit_start, r.id));
            *group_visit_end = std::cmp::max(*group_visit_end, visit_start + r.time);
            active_requests.entry(visit_start + r.time).or_default().push((r.addr, r.size));
        }
    }
}

impl Scheduler for GenerationalScheduler {
    fn name(&self) -> &str {
        "GenerationalScheduler (ScanResistant)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        
        let mut protected_set: HashSet<i64> = HashSet::new();
        let mut hit_count: BTreeMap<i64, u32> = BTreeMap::new();
        
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
            active_requests = active_requests.split_off(&last_rw_end);
            let mut group_visit_end = last_visit_end;
            
            Self::process_group(
                reqs,
                group_indices,
                &mut hbm,
                &mut protected_set,
                &mut hit_count,
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