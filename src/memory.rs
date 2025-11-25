/// 内存管理模块
/// 负责HBM区域的合并、缺失段查找等操作

/// 合并重叠或相邻的内存区域
pub fn merge_regions(regions: &mut Vec<(i64, i64)>) {
    regions.sort_by_key(|r| r.0);
    let mut out = Vec::new();
    for (a, s) in regions.iter() {
        if let Some((la, ls)) = out.last_mut() {
            if *a <= *la + *ls {
                let new_end = std::cmp::max(*la + *ls, *a + *s);
                *ls = new_end - *la;
            } else {
                out.push((*a, *s));
            }
        } else {
            out.push((*a, *s));
        }
    }
    *regions = out;
}

/// 查找HBM中缺失的内存段
/// 
/// # 参数
/// * `hbm` - 当前HBM中已有的内存区域
/// * `addr` - 需要的起始地址
/// * `size` - 需要的大小
/// 
/// # 返回
/// 返回需要加载的内存段列表 (地址, 大小)
pub fn find_missing_segments(hbm: &Vec<(i64, i64)>, addr: i64, size: i64) -> Vec<(i64, i64)> {
    let mut missing = Vec::new();
    let mut cur = addr;
    let end = addr + size;
    let mut regs = hbm.clone();
    regs.sort_by_key(|r| r.0);
    
    for &(ra, rs) in regs.iter() {
        if ra + rs <= cur {
            continue;
        }
        if ra > cur {
            let gap_end = std::cmp::min(ra, end);
            if gap_end > cur {
                missing.push((cur, gap_end - cur));
            }
        }
        cur = std::cmp::max(cur, ra + rs);
        if cur >= end {
            break;
        }
    }
    
    if cur < end {
        missing.push((cur, end - cur));
    }
    
    missing
}


/// 从HBM中移除指定区间，允许部分重叠
pub fn remove_region(regions: &mut Vec<(i64, i64)>, addr: i64, size: i64) {
    if size <= 0 {
        return;
    }
    let end = addr + size;
    let mut updated = Vec::new();
    for &(ra, rs) in regions.iter() {
        let r_end = ra + rs;
        if r_end <= addr || ra >= end {
            updated.push((ra, rs));
            continue;
        }
        if ra < addr {
            updated.push((ra, addr - ra));
        }
        if r_end > end {
            updated.push((end, r_end - end));
        }
    }
    *regions = updated;
}
