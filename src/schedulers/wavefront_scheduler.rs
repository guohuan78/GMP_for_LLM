/// WavefrontScheduler - 时域波前调度
///
/// 创新思路：
/// - 将时间轴按 `wave_span` 切成离散波前，同一波前内的请求统一协调
/// - 每个波前收集需要的缺失段，按“未来波前再使用”距离决定释放哪些区段
/// - 对重叠区段进行切片式卸载（carve & reuse），最大化并行度
///
/// 该策略适合具有阶段性爆发访问、且需要细粒度局部卸载的场景

use crate::memory::{find_missing_segments, merge_regions, remove_region};
use crate::scheduler_trait::Scheduler;
use crate::types::Req;
use std::collections::BTreeMap;
use std::env;

pub struct WavefrontScheduler {
    wave_span: i64,
}

impl WavefrontScheduler {
    pub fn new() -> Self {
        let span = env::var("GMP_WAVE_SPAN")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(4000)
            .max(1);
        Self { wave_span: span }
    }

    fn wave_id(&self, t: i64) -> i64 {
        if t <= 0 {
            0
        } else {
            t / self.wave_span
        }
    }

    fn regions_overlap(a: (i64, i64), b: (i64, i64)) -> bool {
        a.0 < b.0 + b.1 && b.0 < a.0 + a.1
    }

    fn carve_non_overlap(region: (i64, i64), blockers: &[(i64, i64)]) -> Vec<(i64, i64)> {
        let mut segments = vec![region];
        for &(ba, bs) in blockers {
            let b_end = ba + bs;
            let mut next = Vec::new();
            for (sa, ss) in segments.into_iter() {
                let s_end = sa + ss;
                if b_end <= sa || ba >= s_end {
                    next.push((sa, ss));
                    continue;
                }
                if sa < ba {
                    next.push((sa, ba - sa));
                }
                if b_end < s_end {
                    next.push((b_end, s_end - b_end));
                }
            }
            segments = next;
        }
        segments.into_iter().filter(|&(_, s)| s > 0).collect()
    }

    fn estimate_reuse(&self, addr: i64, size: i64, start_idx: usize, reqs: &Vec<Req>) -> i64 {
        let region = (addr, size);
        for r in reqs.iter().skip(start_idx) {
            if Self::regions_overlap(region, (r.addr, r.size)) {
                return self.wave_id(r.start);
            }
        }
        self.wave_id(reqs.last().map(|r| r.start).unwrap_or(0)) + 1000
    }
}

impl Scheduler for WavefrontScheduler {
    fn name(&self) -> &str {
        "WavefrontScheduler (时域波前)"
    }

    fn schedule(&self, reqs: &Vec<Req>, _l: i64, m: i64) -> Result<String, String> {
        let mut output: Vec<String> = Vec::new();
        let mut hbm: Vec<(i64, i64)> = Vec::new();
        let mut last_rw_end: i64 = 0;
        let mut last_visit_end: i64 = 0;
        let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

        let mut i = 0usize;
        while i < reqs.len() {
            let current_wave = self.wave_id(reqs[i].start);
            let mut group_indices: Vec<usize> = Vec::new();
            let mut j = i;
            while j < reqs.len() && self.wave_id(reqs[j].start) == current_wave {
                group_indices.push(j);
                j += 1;
            }

            if group_indices.is_empty() {
                i = j;
                continue;
            }

            active_requests = active_requests.split_off(&last_rw_end);

            let mut temp_hbm = hbm.clone();
            let mut group_loads: Vec<(usize, Vec<(i64, i64)>)> = Vec::new();
            for &req_idx in &group_indices {
                let r = &reqs[req_idx];
                let missing = find_missing_segments(&temp_hbm, r.addr, r.size);
                if !missing.is_empty() {
                    group_loads.push((req_idx, missing.clone()));
                    for seg in missing {
                        temp_hbm.push(seg);
                    }
                    merge_regions(&mut temp_hbm);
                }
            }

            let total_load: i64 = group_loads
                .iter()
                .flat_map(|(_, segs)| segs.iter())
                .map(|&(_, s)| s)
                .sum();
            let mut need = hbm.iter().map(|&(_, s)| s).sum::<i64>() + total_load - m;

            let mut to_offload: Vec<(i64, i64)> = Vec::new();
            if need > 0 {
                let mut group_regions: Vec<(i64, i64)> = group_indices
                    .iter()
                    .map(|&idx| (reqs[idx].addr, reqs[idx].size))
                    .collect();
                merge_regions(&mut group_regions);

                let mut candidates: Vec<(bool, i64, i64, i64)> = hbm
                    .iter()
                    .map(|&(a, s)| {
                        let overlaps_group = group_regions
                            .iter()
                            .any(|&gr| Self::regions_overlap((a, s), gr));
                        let reuse_wave = self.estimate_reuse(a, s, j, reqs);
                        (overlaps_group, reuse_wave, a, s)
                    })
                    .collect();

                candidates.sort_by(|a, b| {
                    a.0.cmp(&b.0)
                        .then(b.1.cmp(&a.1))
                        .then(a.2.cmp(&b.2))
                });

                for (overlap, _reuse, a, s) in candidates {
                    if need <= 0 {
                        break;
                    }
                    if !overlap {
                        let take = std::cmp::min(s, need);
                        to_offload.push((a, take));
                        need -= take;
                    } else {
                        for (sa, ss) in Self::carve_non_overlap((a, s), &group_regions) {
                            if need <= 0 {
                                break;
                            }
                            let take = std::cmp::min(ss, need);
                            to_offload.push((sa, take));
                            need -= take;
                        }
                    }
                }

                if need > 0 {
                    return Err(format!(
                        "Wave {} 无法腾出足够空间（缺 {} 字节）",
                        current_wave, need
                    ));
                }
            }

            if !to_offload.is_empty() {
                let mut start_t = std::cmp::max(last_rw_end, last_visit_end);
                for &(oa, osz) in &to_offload {
                    for (&visit_end, locked) in active_requests.iter() {
                        for &(la, lsz) in locked {
                            if Self::regions_overlap((oa, osz), (la, lsz)) {
                                start_t = std::cmp::max(start_t, visit_end);
                            }
                        }
                    }
                }

                let mut t = start_t;
                for &(oa, osz) in &to_offload {
                    output.push(format!("Offload {} {} {}", t, oa, osz));
                    t += osz * 40;
                    remove_region(&mut hbm, oa, osz);
                }
                last_rw_end = t;
            }

            if !group_loads.is_empty() {
                let total_reload = total_load * 40;
                let anchor = reqs[group_indices[0]].start;
                let reload_start = std::cmp::max(
                    std::cmp::max(last_rw_end, last_visit_end),
                    anchor - total_reload,
                );
                let mut t = reload_start;
                for (_, loads) in &group_loads {
                    for &(la, lsz) in loads {
                        output.push(format!("Reload {} {} {}", t, la, lsz));
                        t += lsz * 40;
                        hbm.push((la, lsz));
                    }
                }
                last_rw_end = t;
                merge_regions(&mut hbm);
            }

            let visit_start = std::cmp::max(
                reqs[group_indices[0]].start,
                std::cmp::max(last_rw_end, last_visit_end),
            );
            for &req_idx in &group_indices {
                let r = &reqs[req_idx];
                output.push(format!("Visit {} {}", visit_start, r.id));
                let visit_end = visit_start + r.time;
                last_visit_end = std::cmp::max(last_visit_end, visit_end);
                active_requests
                    .entry(visit_end)
                    .or_default()
                    .push((r.addr, r.size));
            }

            i = j;
        }

        output.push(format!("Fin {}", last_visit_end));
        Ok(output.join("
"))
    }
}
