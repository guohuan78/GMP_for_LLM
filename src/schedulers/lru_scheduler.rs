/// LRU 调度器（Least Recently Used）
/// 采用最近最少使用策略进行内存替换

use std::collections::BTreeMap;
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments};

pub struct LruScheduler;

impl LruScheduler {
    pub fn new() -> Self {
        Self
    }
    
    /// 处理单个请求的 LRU 逻辑
    fn process_request(
        r: &Req,
        hbm: &mut Vec<(i64, i64)>,
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        access_time: &mut BTreeMap<(i64, i64), i64>,  // 记录每个区域的最后访问时间
        m: i64,
        current_time: i64,
    ) {
        // 清理过期的活跃请求
        *active_requests = active_requests.split_off(last_rw_end);
        
        // 检测缺失段
        let to_load = find_missing_segments(hbm, r.addr, r.size);
        let total_load: i64 = to_load.iter().map(|&(_, s)| s).sum();

        // LRU 策略卸载
        let mut to_offload: Vec<(i64, i64)> = Vec::new();
        if total_load > 0 {
            let cur_hbm_size: i64 = hbm.iter().map(|&(_, s)| s).sum();
            let mut need = (cur_hbm_size + total_load) - m;
            
            if need > 0 {
                // 按最后访问时间排序（越早访问的越优先卸载）
                let mut candidates: Vec<((i64, i64), i64)> = hbm
                    .iter()
                    .map(|&region| {
                        let last_access = access_time.get(&region).copied().unwrap_or(0);
                        (region, last_access)
                    })
                    .collect();
                
                candidates.sort_by_key(|(_, last_access)| *last_access);
                
                for &((a, s), _) in &candidates {
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
                    } else {
                        // 部分卸载：只卸载非重叠部分
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

        // 执行卸载
        if !to_offload.is_empty() {
            let mut start_t = *last_rw_end;
            
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
                access_time.remove(&(oa, osz));
            }
            *last_rw_end = t;
        }

        // 执行加载
        if !to_load.is_empty() {
            let total_reload = total_load * 40;
            let reload_start = std::cmp::max(*last_rw_end, r.start - total_reload);
            let mut t = reload_start;
            
            for &(la, lsz) in &to_load {
                output.push(format!("Reload {} {} {}", t, la, lsz));
                t += lsz * 40;
                hbm.push((la, lsz));
                access_time.insert((la, lsz), current_time);
            }
            *last_rw_end = t;
            merge_regions(hbm);
        }

        // 执行访存
        let visit_start = std::cmp::max(r.start, std::cmp::max(*last_rw_end, *last_visit_end));
        output.push(format!("Visit {} {}", visit_start, r.id));
        let visit_end = visit_start + r.time;
        *last_visit_end = visit_end;
        
        // 更新访问时间
        for &(ha, hs) in hbm.iter() {
            let h_end = ha + hs;
            let r_end = r.addr + r.size;
            if ha < r_end && h_end > r.addr {
                access_time.insert((ha, hs), current_time);
            }
        }
        
        active_requests.entry(visit_end).or_default().push((r.addr, r.size));
    }
}

impl Scheduler for LruScheduler {
    fn name(&self) -> &str {
        "LruScheduler (LRU)"
    }
    
    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();
        let mut access_time: BTreeMap<(i64, i64), i64> = BTreeMap::new();

        for (idx, r) in reqs.iter().enumerate() {
            Self::process_request(
                r,
                &mut hbm,
                &mut output,
                &mut last_rw_end,
                &mut last_visit_end,
                &mut active_requests,
                &mut access_time,
                m,
                idx as i64,  // 使用请求索引作为时间戳
            );
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}
