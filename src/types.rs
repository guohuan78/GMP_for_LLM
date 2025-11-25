/// 公共类型定义

#[derive(Debug, Clone)]
pub struct Req {
    pub addr: i64,
    pub size: i64,
    pub start: i64,
    pub time: i64,
    pub id: usize,
}

/// 解析输入字符串
pub fn parse_input(input: &str) -> Result<(i64, i64, Vec<Req>), String> {
    let mut it = input.split_whitespace();
    
    let l: i64 = it.next()
        .ok_or("缺少 L 参数")?
        .parse()
        .map_err(|_| "L 解析失败")?;
    
    let m: i64 = it.next()
        .ok_or("缺少 M 参数")?
        .parse()
        .map_err(|_| "M 解析失败")?;
    
    let n: usize = it.next()
        .ok_or("缺少 N 参数")?
        .parse()
        .map_err(|_| "N 解析失败")?;
    
    let mut reqs: Vec<Req> = Vec::new();
    for id in 0..n {
        let addr: i64 = it.next()
            .ok_or(format!("请求 {} 缺少 addr", id))?
            .parse()
            .map_err(|_| format!("请求 {} addr 解析失败", id))?;
        
        let size: i64 = it.next()
            .ok_or(format!("请求 {} 缺少 size", id))?
            .parse()
            .map_err(|_| format!("请求 {} size 解析失败", id))?;
        
        let start: i64 = it.next()
            .ok_or(format!("请求 {} 缺少 start", id))?
            .parse()
            .map_err(|_| format!("请求 {} start 解析失败", id))?;
        
        let time: i64 = it.next()
            .ok_or(format!("请求 {} 缺少 time", id))?
            .parse()
            .map_err(|_| format!("请求 {} time 解析失败", id))?;
        
        reqs.push(Req { addr, size, start, time, id });
    }
    
    Ok((l, m, reqs))
}
