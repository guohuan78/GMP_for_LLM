/// Base 调度器（简单实现，用于对比）
/// 采用最简单的 FIFO 策略，不使用 Bélády 算法

use std::collections::BTreeMap;
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments};

pub struct BaseScheduler;

impl BaseScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl Scheduler for BaseScheduler {
    fn name(&self) -> &str {
        "BaseScheduler (FIFO)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

        for r in reqs {
            // 清理过期的活跃请求
            active_requests = active_requests.split_off(&last_rw_end);
            
            // 检测缺失段
            let to_load = find_missing_segments(&hbm, r.addr, r.size);
            let total_load: i64 = to_load.iter().map(|&(_, s)| s).sum();

            // 简单的卸载策略：如果空间不足，按 FIFO 顺序卸载
            let mut to_offload: Vec<(i64, i64)> = Vec::new();
            if total_load > 0 {
                let cur_hbm_size: i64 = hbm.iter().map(|&(_, s)| s).sum();
                let mut need = (cur_hbm_size + total_load) - m;
                
                if need > 0 {
                    // FIFO：按添加顺序卸载
                    for &(a, s) in hbm.iter() {
                        if need <= 0 {
                            break;
                        }
                        
                        // 检查是否与当前请求重叠
                        let r_end = r.addr + r.size;
                        let region_end = a + s;
                        let overlaps = a < r_end && region_end > r.addr;
                        
                        if !overlaps {
                            let take = std::cmp::min(s, need);
                            to_offload.push((a, take));
                            need -= take;
                        }
                    }
                }
            }

            // 执行卸载
            if !to_offload.is_empty() {
                let mut start_t = last_rw_end;
                
                // 检查活跃区域冲突
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
                last_rw_end = t;
            }

            // 执行加载
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

            // 执行访存
            let visit_start = std::cmp::max(r.start, std::cmp::max(last_rw_end, last_visit_end));
            output.push(format!("Visit {} {}", visit_start, r.id));
            let visit_end = visit_start + r.time;
            last_visit_end = visit_end;
            active_requests.entry(visit_end).or_default().push((r.addr, r.size));
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}
