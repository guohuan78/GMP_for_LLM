/// 调度器 Trait 定义
/// 所有调度算法必须实现此接口

use crate::types::Req;

/// 调度器统一接口
pub trait Scheduler: Send + Sync {
    /// 调度器名称，用于日志和调试
    fn name(&self) -> &str;
    
    /// 核心调度方法
    /// 
    /// # 参数
    /// * `reqs` - 请求序列（已按 start 排序）
    /// * `l` - 虚拟地址空间大小
    /// * `m` - HBM 容量
    /// 
    /// # 返回
    /// * `Ok(String)` - 符合格式的操作序列（包含 Fin）
    /// * `Err(String)` - 调度失败的错误信息
    fn schedule(&self, reqs: &Vec<Req>, l: i64, m: i64) -> Result<String, String>;
}

/// 调度结果评估器
pub struct ScheduleResult {
    pub scheduler_name: String,
    pub output: String,
    pub fin_time: i64,
    pub is_valid: bool,
    pub error_msg: Option<String>,
}

impl ScheduleResult {
    /// 从输出字符串创建结果
    pub fn from_output(scheduler_name: String, output: String) -> Self {
        let (is_valid, fin_time, error_msg) = Self::validate_and_extract_fin(&output);
        
        Self {
            scheduler_name,
            output,
            fin_time,
            is_valid,
            error_msg,
        }
    }
    
    /// 从错误创建结果
    pub fn from_error(scheduler_name: String, error: String) -> Self {
        Self {
            scheduler_name,
            output: String::new(),
            fin_time: i64::MAX,
            is_valid: false,
            error_msg: Some(error),
        }
    }
    
    /// 验证输出并提取 Fin 时间
    fn validate_and_extract_fin(output: &str) -> (bool, i64, Option<String>) {
        let lines: Vec<&str> = output.lines().collect();
        
        if lines.is_empty() {
            return (false, i64::MAX, Some("输出为空".to_string()));
        }
        
        // 检查最后一行是否为 Fin
        let last_line = lines.last().unwrap();
        if !last_line.starts_with("Fin ") {
            return (false, i64::MAX, Some("最后一行不是 Fin".to_string()));
        }
        
        // 提取 Fin 时间
        let fin_time = last_line
            .strip_prefix("Fin ")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(i64::MAX);
        
        if fin_time == i64::MAX {
            return (false, i64::MAX, Some("无法解析 Fin 时间".to_string()));
        }
        
        // 基本格式检查（可扩展）
        for (i, line) in lines.iter().enumerate() {
            if i == lines.len() - 1 {
                break; // 最后一行已检查
            }
            
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                return (false, fin_time, Some(format!("第{}行格式错误", i + 1)));
            }
            
            match parts[0] {
                "Reload" | "Offload" => {
                    if parts.len() != 4 {
                        return (false, fin_time, Some(format!("第{}行 {} 参数数量错误", i + 1, parts[0])));
                    }
                }
                "Visit" => {
                    if parts.len() != 3 {
                        return (false, fin_time, Some(format!("第{}行 Visit 参数数量错误", i + 1)));
                    }
                }
                _ => {
                    return (false, fin_time, Some(format!("第{}行操作类型未知: {}", i + 1, parts[0])));
                }
            }
        }
        
        (true, fin_time, None)
    }
}

/// 调度器选择策略
#[allow(dead_code)]
pub enum SelectionStrategy {
    /// 选择 Fin 时间最短的合法结果
    BestFinTime,
    /// 选择第一个合法结果
    FirstValid,
}

/// 调度结果选择器
pub struct ResultSelector {
    strategy: SelectionStrategy,
}

impl ResultSelector {
    pub fn new(strategy: SelectionStrategy) -> Self {
        Self { strategy }
    }
    
    /// 从多个结果中选择最优的
    pub fn select_best<'a>(&self, results: &'a [ScheduleResult]) -> Option<&'a ScheduleResult> {
        match self.strategy {
            SelectionStrategy::BestFinTime => {
                results
                    .iter()
                    .filter(|r| r.is_valid)
                    .min_by_key(|r| r.fin_time)
            }
            SelectionStrategy::FirstValid => {
                results.iter().find(|r| r.is_valid)
            }
        }
    }
}
