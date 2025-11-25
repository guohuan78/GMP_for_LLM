/// SlidingWindowGreedyScheduler - 滑动窗口贪心策略
/// 
/// 核心思想：
/// - 在未来 W 个请求中查找下一次使用时间（Bélády 的近似）
/// - 避免全序列扫描，提升大规模输入下的效率
/// - 默认窗口大小 W = 200，可通过环境变量 GMP_WINDOW 覆盖
use std::collections::BTreeMap;
use std::env;

use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use crate::memory::{merge_regions, find_missing_segments, remove_region};

pub struct SlidingWindowGreedyScheduler {
    window_size: usize,
}

impl SlidingWindowGreedyScheduler {
    pub fn new() -> Self {
        let window_size = env::var("GMP_WINDOW")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(200); // 默认窗口大小 200
        Self { window_size }
    }

    /// 在 [current_idx + 1, current_idx + window_size) 范围内查找下一次使用时间
    fn find_next_use(&self, addr: i64, size: i64, current_idx: usize, reqs: &Vec<Req>) -> i64 {
        let region_end = addr + size;
        let end = std::cmp::min(reqs.len(), current_idx + self.window_size);
        for i in (current_idx + 1)..end {
            let r = &reqs[i];
            if std::cmp::max(addr, r.addr) < std::cmp::min(region_end, r.addr + r.size) {
                return r.start;
            }
        }
        // 超出窗口或未找到 → 设为极大值（优先卸载）
        i64::MAX / 4
    }

    /// 处理同时开始的一组请求（组级别 IO 合并）
    fn process_simultaneous_group(
        &self,
        reqs: &Vec<Req>,
        group_indices: &Vec<usize>,
        hbm: &mut Vec<(i64, i64)>,
        output: &mut Vec<String>,
        last_rw_end: &mut i64,
        last_visit_end: &mut i64,
        group_visit_end: &mut i64,
        active_requests: &mut BTreeMap<i64, Vec<(i64, i64)>>,
        m: i64,
        base_idx: usize,
    ) {
        // Step 1: 收集所有缺失段（模拟加载后状态）
        let mut temp_hbm = hbm.clone();
        let mut all_loads: Vec<(usize, Vec<(i64, i64)>)> = Vec::new();
        for &idx in group_indices {
            let r = &reqs[idx];
            let missing = find_missing_segments(&temp_hbm, r.addr, r.size);
            if !missing.is_empty() {
                all_loads.push((idx, missing.clone()));
                for seg in &missing {
                    temp_hbm.push(*seg);
                }
                merge_regions(&mut temp_hbm);
            }
        }

        let total_load_size: i64 = all_loads.iter().flat_map(|(_, segs)| segs.iter().map(|&(_, s)| s)).sum();

        // Step 2: 若需卸载，选择未来最久不用的非活跃、非当前组区域
        let mut to_offload: Vec<(i64, i64)> = Vec::new();
        if total_load_size > 0 {
            let current_usage: i64 = hbm.iter().map(|&(_, s)| s).sum();
            let mut need = current_usage + total_load_size - m;
            if need > 0 {
                // 构建候选列表：(next_use_time, addr, size)
                let mut candidates: Vec<(i64, i64, i64)> = Vec::new();
                for &(a, s) in hbm.iter() {
                    let next_use = self.find_next_use(a, s, base_idx, reqs);
                    candidates.push((next_use, a, s));
                }
                // 按 next_use 降序（最久不用排前面）
                candidates.sort_by_key(|k| std::cmp::Reverse(k.0));

                // 合并当前组所有区域，用于重叠检测
                let mut group_regions: Vec<(i64, i64)> = group_indices.iter().map(|&i| (reqs[i].addr, reqs[i].size)).collect();
                merge_regions(&mut group_regions);

                // 选择可卸载区域
                for &(_, a, s) in &candidates {
                    if need <= 0 { break; }

                    // 检查是否与当前组任何区域重叠
                    let mut overlaps_group = false;
                    for &(ga, gs) in &group_regions {
                        if a < ga + gs && a + s > ga {
                            overlaps_group = true;
                            break;
                        }
                    }
                    if overlaps_group { continue; }

                    // 检查是否在活跃请求中（不能卸载）
                    let mut is_active = false;
                    for locked_regions in active_requests.values() {
                        for &(la, ls) in locked_regions {
                            if a < la + ls && a + s > la {
                                is_active = true;
                                break;
                            }
                        }
                        if is_active { break; }
                    }
                    if is_active { continue; }

                    let take = std::cmp::min(s, need);
                    to_offload.push((a, take));
                    need -= take;
                }

                if need > 0 {
                    // 无法腾出足够空间
                    // println!("SlidingWindowGreedy: Cannot free enough space for group starting at idx {}", base_idx);
                }
            }
        }

        // Step 3: 执行 Offload
        if !to_offload.is_empty() {
            let mut start_t = std::cmp::max(*last_rw_end, *last_visit_end);
            // 确保不与活跃区域冲突
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
                remove_region(hbm, oa, osz);
            }
            *last_rw_end = t;
        }

        // Step 4: 执行 Reload
        if !all_loads.is_empty() {
            let group_start_time = reqs[group_indices[0]].start;
            let total_io_time = total_load_size * 40;
            let earliest_start = group_start_time.saturating_sub(total_io_time);
            let reload_start = std::cmp::max(*last_rw_end, std::cmp::max(*last_visit_end, earliest_start));
            let mut t = reload_start;
            for (_, segs) in &all_loads {
                for &(a, s) in segs {
                    output.push(format!("Reload {} {} {}", t, a, s));
                    t += s * 40;
                    hbm.push((a, s));
                }
            }
            *last_rw_end = t;
            merge_regions(hbm);
        }

        // Step 5: 统一执行 Visit（同时开始）
        let group_req_start = reqs[group_indices[0]].start;
        let visit_start = std::cmp::max(group_req_start, std::cmp::max(*last_rw_end, *last_visit_end));
        for &idx in group_indices {
            let r = &reqs[idx];
            output.push(format!("Visit {} {}", visit_start, r.id));
            let visit_end = visit_start + r.time;
            *group_visit_end = std::cmp::max(*group_visit_end, visit_end);
            active_requests.entry(visit_end).or_default().push((r.addr, r.size));
        }
    }
}

impl Scheduler for SlidingWindowGreedyScheduler {
    fn name(&self) -> &str {
        "SlidingWindowGreedyScheduler"
    }

    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        // let debug = env::var("GMP_DEBUG").map(|v| v == "1").unwrap_or(false);
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

        let mut i = 0;
        while i < reqs.len() {
            let current_start = reqs[i].start;
            let mut j = i + 1;
            while j < reqs.len() && reqs[j].start == current_start {
                j += 1;
            }
            // 清理已结束的活跃请求
            active_requests = active_requests.split_off(&last_rw_end);

            let group_indices: Vec<usize> = (i..j).collect();
            let is_group = group_indices.len() > 1;

            let mut group_visit_end = last_visit_end;
            if is_group {
                self.process_simultaneous_group(
                    reqs,
                    &group_indices,
                    &mut hbm,
                    &mut output,
                    &mut last_rw_end,
                    &mut last_visit_end,
                    &mut group_visit_end,
                    &mut active_requests,
                    m,
                    i,
                );
            } else {
                // 单请求处理（复用组逻辑简化版）
                self.process_simultaneous_group(
                    reqs,
                    &group_indices,
                    &mut hbm,
                    &mut output,
                    &mut last_rw_end,
                    &mut last_visit_end,
                    &mut group_visit_end,
                    &mut active_requests,
                    m,
                    i,
                );
            }

            last_visit_end = group_visit_end;
            i = j;
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("\n"))
    }
}