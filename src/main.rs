use std::collections::BTreeMap;
use std::io::{self, BufRead};

mod tests;

// --- Data Structures ---

#[derive(Debug, Clone)]
struct Req {
    id: usize,
    addr: i64,
    size: i64,
    start: i64,
    time: i64,
}

#[derive(Debug, Clone)]
struct Input {
    l: i64,
    m: i64,
    reqs: Vec<Req>,
}

// --- Input Parsing (Unchanged from original) ---

fn read_and_parse_input() -> Input {
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    let mut first_line = String::new();
    handle.read_line(&mut first_line).unwrap();

    let parts: Vec<&str> = first_line.trim().split_whitespace().collect();
    if parts.len() < 3 {
        panic!("Invalid input format");
    }

    let l: i64 = parts[0].parse().unwrap();
    let m: i64 = parts[1].parse().unwrap();
    let n: usize = parts[2].parse().unwrap();

    let mut reqs: Vec<Req> = Vec::new();
    for i in 0..n {
        let mut line = String::new();
        handle.read_line(&mut line).unwrap();

        let nums: Vec<&str> = line.trim().split_whitespace().collect();
        if nums.len() >= 4 {
            let addr: i64 = nums[0].parse().unwrap();
            let size: i64 = nums[1].parse().unwrap();
            let start: i64 = nums[2].parse().unwrap();
            let time: i64 = nums[3].parse().unwrap();

            reqs.push(Req {
                id: i,
                addr,
                size,
                start,
                time,
            });
        }
    }

    Input { l, m, reqs }
}

fn parse_input(input: &str) -> Input {
    let mut it = input.split_whitespace();

    let l: i64 = it.next().unwrap().parse().unwrap();
    let m: i64 = it.next().unwrap().parse().unwrap();
    let n: usize = it.next().unwrap().parse().unwrap();

    let mut reqs: Vec<Req> = Vec::new();
    for i in 0..n {
        let addr: i64 = it.next().unwrap().parse().unwrap();
        let size: i64 = it.next().unwrap().parse().unwrap();
        let start: i64 = it.next().unwrap().parse().unwrap();
        let time: i64 = it.next().unwrap().parse().unwrap();

        reqs.push(Req {
            id: i,
            addr,
            size,
            start,
            time,
        });
    }

    Input { l, m, reqs }
}

// --- Core Logic: Helper Functions ---

/// Merges overlapping or adjacent intervals in a vector.
/// Ensures the representation of HBM memory is always compact and disjoint.
fn merge_regions(regions: &mut Vec<(i64, i64)>) {
    if regions.is_empty() {
        return;
    }

    regions.sort_by_key(|k| k.0);
    let mut merged = Vec::with_capacity(regions.len());
    merged.push(regions[0]);

    for &(addr, size) in regions.iter().skip(1) {
        let last = merged.last_mut().unwrap();
        let end = addr + size;
        let last_end = last.0 + last.1;

        if addr <= last_end {
            // Overlap or adjacent, merge them
            if end > last_end {
                last.1 = end - last.0;
            }
        } else {
            // Disjoint, add new region
            merged.push((addr, size));
        }
    }

    *regions = merged;
}

/// Finds which parts of a required memory segment are not currently in HBM.
fn find_missing_segments(hbm_regions: &[(i64, i64)], req_addr: i64, req_size: i64) -> Vec<(i64, i64)> {
    let mut missing = Vec::new();
    let req_end = req_addr + req_size;
    let mut current_pos = req_addr;

    for &(loaded_addr, loaded_size) in hbm_regions {
        if current_pos >= req_end {
            break;
        }

        let loaded_end = loaded_addr + loaded_size;

        // Check for a gap before the current loaded region
        if current_pos < loaded_addr {
            let gap_end = std::cmp::min(loaded_addr, req_end);
            missing.push((current_pos, gap_end - current_pos));
        }

        // Move our position past the current loaded region
        if loaded_end > current_pos {
            current_pos = loaded_end;
        }
    }

    // Check for any remaining gap at the end
    if current_pos < req_end {
        missing.push((current_pos, req_end - current_pos));
    }

    missing
}

/// Implements the core of Bélády's algorithm.
/// For a given memory region, it looks into the future to find when it will be used next.
fn find_next_use(addr: i64, size: i64, current_req_idx: usize, all_reqs: &[Req]) -> i64 {
    let end = addr + size;

    for req in all_reqs.iter().skip(current_req_idx + 1) {
        let req_end = req.addr + req.size;

        // Check for overlap
        if std::cmp::max(addr, req.addr) < std::cmp::min(end, req_end) {
            return req.start;
        }
    }

    i64::MAX // If never used again, it's the best candidate for eviction
}

// --- Main Solver ---

fn solve(data: &Input) -> String {
    let mut output = Vec::new();
    let mut last_rw_end: i64 = 0;
    let mut last_visit_end: i64 = 0;

    // Tracks disjoint, sorted memory regions in HBM: Vec<(addr, size)>
    let mut hbm_regions: Vec<(i64, i64)> = Vec::new();

    // Tracks requests currently in their 'Visit' phase to prevent eviction of their memory.
    // BTreeMap<end_time, (addr, size)>
    let mut active_requests: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();

    for (i, r) in data.reqs.iter().enumerate() {
        // --- 1. Update State: Unlock memory from requests that have finished by now ---
        let current_time = std::cmp::max(last_rw_end, last_visit_end);
        active_requests = active_requests.split_off(&current_time);

        // --- 2. Analysis Phase: Determine what to load and what to offload ---
        let to_load = find_missing_segments(&hbm_regions, r.addr, r.size);
        let total_load_size: i64 = to_load.iter().map(|&(_, s)| s).sum();

        let mut to_offload: Vec<(i64, i64)> = Vec::new();

        if total_load_size > 0 {
            let current_hbm_size: i64 = hbm_regions.iter().map(|&(_, s)| s).sum();
            let mut space_needed_to_free = (current_hbm_size + total_load_size) - data.m;

            if space_needed_to_free > 0 {
                // --- Bélády's Optimal Eviction Logic ---
                let mut candidates = Vec::new();

                'region_loop: for &(addr, size) in &hbm_regions {
                    // Check if the region is locked by an active request
                    for locked_regions in active_requests.values() {
                        for &(locked_addr, locked_size) in locked_regions {
                            if std::cmp::max(addr, locked_addr)
                                < std::cmp::min(addr + size, locked_addr + locked_size)
                            {
                                continue 'region_loop; // This region is locked, skip it
                            }
                        }
                    }

                    // If not locked, it's a candidate for eviction
                    let next_use = find_next_use(addr, size, i, &data.reqs);
                    candidates.push((next_use, addr, size));
                }

                // Sort candidates: furthest next use time first (descending)
                candidates.sort_by_key(|k| -k.0);

                for (_, addr, size) in candidates {
                    if space_needed_to_free <= 0 {
                        break;
                    }

                    to_offload.push((addr, size));
                    space_needed_to_free -= size;
                }
            }
        }

        // --- 3. Scheduling Phase: Schedule I/O and Visit operations ---

        // Schedule Offloads
        if !to_offload.is_empty() {
            let offload_start_time = last_rw_end;
            let mut current_offload_time = offload_start_time;

            for &(addr, size) in &to_offload {
                let offload_duration = size * 40;
                output.push(format!("Offload {} {} {}", current_offload_time, addr, size));
                current_offload_time += offload_duration;
            }

            last_rw_end = current_offload_time;

            // Update HBM state
            hbm_regions.retain(|r| !to_offload.contains(r));
        }

        // Schedule Reloads (Just-in-Time)
        if !to_load.is_empty() {
            let total_reload_duration = total_load_size * 40;

            // JIT: Aim to finish reload right at r.start, but not before I/O channel is free.
            let reload_start_time = std::cmp::max(last_rw_end, r.start - total_reload_duration);
            let mut current_reload_time = reload_start_time;

            for &(addr, size) in &to_load {
                let reload_duration = size * 40;
                output.push(format!("Reload {} {} {}", current_reload_time, addr, size));
                current_reload_time += reload_duration;
                hbm_regions.push((addr, size));
            }

            last_rw_end = current_reload_time;
            merge_regions(&mut hbm_regions);
        }

        // Schedule Visit
        let visit_start_time = std::cmp::max(r.start, std::cmp::max(last_visit_end, last_rw_end));
        output.push(format!("Visit {} {}", visit_start_time, r.id));

        let visit_end_time = visit_start_time + r.time;
        last_visit_end = visit_end_time;

        // Lock the memory for this new active request
        active_requests.entry(visit_end_time).or_default().push((r.addr, r.size));
    }

    let fin_time = std::cmp::max(last_rw_end, last_visit_end);
    output.push(format!("Fin {}", fin_time));

    output.join("\n")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "test" {
        tests::run_tests(|input| {
            let data = parse_input(input);
            solve(&data)
        });
    } else {
        let data = read_and_parse_input();
        let result = solve(&data);
        println!("{}", result);
    }
}
