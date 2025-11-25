/// LazyScheduler - 延迟/保守策略
/// 
/// 核心思想：
/// - 不到万不得已不加载，用完立刻释放
/// - 严格按需加载，最小化内存占用
/// - 积极释放数据，避免内存溢出
/// 
/// 适用场景：
/// - 内存极其紧张（M很小）
/// - 数据访问模式不规律，预取收益低

use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments};
use std::collections::BTreeMap;

pub struct LazyScheduler;

impl LazyScheduler {
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

impl Scheduler for LazyScheduler {
    fn name(&self) -> &str {
        "LazyScheduler (延迟保守)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let verbose = std::env::var("GMP_VERBOSE").map(|v| v == "1").unwrap_or(false);
        
        if verbose {
            eprintln!("[Lazy] 开始调度，M={}, N={}", m, reqs.len());
        }
        
        let mut output = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end = 0i64;
        let mut last_visit_end = 0i64;
        
        // 活跃请求：访问结束时间 -> 数据区域列表
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();
        
        let mut reload_count = 0;
        let mut offload_count = 0;
        let mut eager_offload_count = 0;
        
        for (idx, req) in reqs.iter().enumerate() {
            // 积极清理：立即释放过期的数据（延迟策略核心）
            let expired: Vec<i64> = active_requests
                .range(..=last_visit_end)
                .map(|(k, _)| *k)
                .collect();
            
            for key in expired {
                if let Some(regions) = active_requests.remove(&key) {
                    for (ea, es) in regions {
                        // 检查是否还有其他活跃请求需要这个区域
                        let still_needed = active_requests.values().any(|regs| {
                            regs.iter().any(|r| self.regions_overlap(&(ea, es), r))
                        });
                        
                        if !still_needed && hbm.contains(&(ea, es)) {
                            // 立即释放（延迟策略核心特征）
                            output.push(format!("Offload {} {} {}", last_rw_end, ea, es));
                            last_rw_end += es;
                            hbm.retain(|&r| r != (ea, es));
                            eager_offload_count += 1;
                        }
                    }
                }
            }
            
            // 检查需要加载的数据
            let missing = find_missing_segments(&hbm, req.addr, req.size);
            
            if !missing.is_empty() {
                let need_size: i64 = missing.iter().map(|(_, s)| s).sum();
                let current_usage = self.calculate_hbm_usage(&hbm);
                
                // 如果空间不足，激进地释放空间
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
                    
                    // 按大小降序排序，优先释放大块（释放更多空间）
                    to_offload.sort_by_key(|(_, s)| -s);
                    
                    let mut freed = 0i64;
                    for (ha, hs) in to_offload {
                        if current_usage - freed + need_size <= m {
                            break;
                        }
                        
                        output.push(format!("Offload {} {} {}", last_rw_end, ha, hs));
                        last_rw_end += hs;
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
                let mut t = last_rw_end.max(req.start);
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
                "[Lazy] 完成：Reload={}, Offload={}, Fin={}",
                reload_count, offload_count + eager_offload_count, fin_time
            );
        }
        
        Ok(output.join("\n"))
    }
}
