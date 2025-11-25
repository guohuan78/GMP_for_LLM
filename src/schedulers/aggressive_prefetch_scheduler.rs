/// AggressivePrefetchScheduler - 激进预取策略 (简化版)
/// 
/// 核心思想：
/// - 尽早加载数据，利用空闲时间预取
/// - 只要内存够用，就提前加载下一个请求的数据
/// - 保守地释放数据，尽量延长数据在HBM中的停留时间
/// 
/// 适用场景：
/// - 内存相对充裕（M较大）
/// - 数据复用率高的场景

use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments};
use std::collections::BTreeMap;

pub struct AggressivePrefetchScheduler;

impl AggressivePrefetchScheduler {
    pub fn new() -> Self {
        Self
    }
    
    fn regions_overlap(&self, a: &(i64, i64), b: &(i64, i64)) -> bool {
        a.0 < b.0 + b.1 && b.0 < a.0 + a.1
    }
    
    fn calculate_hbm_usage(&self, hbm: &Vec<(i64, i64)>) -> i64 {
        hbm.iter().map(|(_, s)| s).sum()
    }
}

impl Scheduler for AggressivePrefetchScheduler {
    fn name(&self) -> &str {
        "AggressivePrefetchScheduler (激进预取)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let verbose = std::env::var("GMP_VERBOSE").map(|v| v == "1").unwrap_or(false);
        
        if verbose {
            eprintln!("[AggressivePrefetch] 开始调度，M={}, N={}", m, reqs.len());
        }
        
        let mut output = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end = 0i64;
        let mut last_visit_end = 0i64;
        
        // 活跃请求：访问结束时间 -> 数据区域列表
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();
        
        let mut reload_count = 0;
        let mut offload_count = 0;
        
        for (idx, req) in reqs.iter().enumerate() {
            // 清理过期的活跃请求
            active_requests = active_requests.split_off(&last_visit_end);
            
            // 检查需要加载的数据
            let missing = find_missing_segments(&hbm, req.addr, req.size);
            
            if !missing.is_empty() {
                let need_size: i64 = missing.iter().map(|(_, s)| s).sum();
                let current_usage = self.calculate_hbm_usage(&hbm);
                
                // 如果空间不足，需要offload
                // 激进策略：只释放必要的空间（最小化offload）
                if current_usage + need_size > m {
                    let mut to_offload = Vec::new();
                    
                    for &(ha, hs) in hbm.iter() {
                        let is_active = active_requests.values().any(|regions| {
                            regions.iter().any(|r| self.regions_overlap(&(ha, hs), r))
                        });
                        
                        let is_current = self.regions_overlap(&(ha, hs), &(req.addr, req.size));
                        
                        if !is_active && !is_current {
                            to_offload.push((ha, hs));
                        }
                    }
                    
                    // 按大小升序排序，优先释放小块（保留大块，可能未来有用）
                    to_offload.sort_by_key(|(_, s)| *s);
                    
                    let mut freed = 0i64;
                    for (ha, hs) in to_offload {
                        if current_usage - freed + need_size <= m {
                            break;
                        }
                        
                        let start_t = last_rw_end.max(req.start);
                        output.push(format!("Offload {} {} {}", start_t, ha, hs));
                        last_rw_end = start_t + hs;
                        freed += hs;
                        offload_count += 1;
                        
                        hbm.retain(|&r| r != (ha, hs));
                    }
                    
                    if self.calculate_hbm_usage(&hbm) + need_size > m {
                        return Err(format!(
                            "请求 {} 无法腾出足够空间（需要 {}，当前 {}，M={}）",
                            idx, need_size, self.calculate_hbm_usage(&hbm), m
                        ));
                    }
                }
                
                // 加载数据
                let start_t = last_rw_end.max(req.start);
                let mut t = start_t;
                for (a, s) in missing {
                    output.push(format!("Reload {} {} {}", t, a, s));
                    t += s;
                    hbm.push((a, s));
                    reload_count += 1;
                }
                last_rw_end = t;
                merge_regions(&mut hbm);
            }
            
            // Visit操作
            let visit_start = req.start.max(last_rw_end).max(last_visit_end);
            output.push(format!("Visit {} {}", visit_start, idx));
            let visit_end = visit_start + req.time;
            last_visit_end = last_visit_end.max(visit_end);
            
            active_requests.entry(visit_end).or_default().push((req.addr, req.size));
        }
        
        // 计算Fin时间
        let fin_time = output
            .iter()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    parts[1].parse::<i64>().ok()
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0);
        
        output.push(format!("Fin {}", fin_time));
        
        if verbose {
            eprintln!(
                "[AggressivePrefetch] 完成：Reload={}, Offload={}, Fin={}",
                reload_count, offload_count, fin_time
            );
        }
        
        Ok(output.join("\n"))
    }
}
