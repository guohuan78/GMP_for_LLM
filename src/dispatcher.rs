/// 调度器调度器（Dispatcher）
/// 负责管理多个调度策略，并选择最优结果

use std::env;
use std::process::Command;
use std::path::Path;
use crate::scheduler_trait::{Scheduler, ScheduleResult, ResultSelector, SelectionStrategy};
use crate::types::parse_input;
use crate::schedulers::{AggressivePrefetchScheduler, BaseScheduler, GreedyScheduler, LazyScheduler, LruScheduler, OverlapAwareEvictionScheduler, SlidingWindowGreedyScheduler, WavefrontScheduler};

/// 调度器注册表
pub struct SchedulerRegistry {
    schedulers: Vec<Box<dyn Scheduler>>,
}

impl SchedulerRegistry {
    /// 创建并注册所有调度器
    pub fn new() -> Self {
        let mut schedulers: Vec<Box<dyn Scheduler>> = Vec::new();
        
        // 注册调度器（按优先级顺序）
        // 添加新调度器：只需在此处 push 即可
        schedulers.push(Box::new(GreedyScheduler::new()));
        schedulers.push(Box::new(WavefrontScheduler::new()));
        schedulers.push(Box::new(LruScheduler::new()));
        schedulers.push(Box::new(AggressivePrefetchScheduler::new()));
        schedulers.push(Box::new(LazyScheduler::new()));
        schedulers.push(Box::new(BaseScheduler::new()));
        schedulers.push(Box::new(SlidingWindowGreedyScheduler::new()));
        schedulers.push(Box::new(OverlapAwareEvictionScheduler::new()));
        
        Self { schedulers }
    }
    
    /// 获取所有调度器
    pub fn get_schedulers(&self) -> &[Box<dyn Scheduler>] {
        &self.schedulers
    }
}

/// 主调度器（Dispatcher）
pub struct Dispatcher {
    registry: SchedulerRegistry,
    selector: ResultSelector,
    verbose: bool,
    use_checker: bool,
}

impl Dispatcher {
    /// 创建新的调度器
    pub fn new() -> Self {
        let verbose = env::var("GMP_VERBOSE").map(|v| v == "1").unwrap_or(false);
        let checker_path = Path::new("checker/checker");
        let use_checker = checker_path.exists();
        
        if verbose && use_checker {
            eprintln!("✓ Checker 已启用: checker/checker");
        } else if verbose {
            eprintln!("⚠ Checker 未找到，将跳过验证");
        }
        
        Self {
            registry: SchedulerRegistry::new(),
            selector: ResultSelector::new(SelectionStrategy::BestFinTime),
            verbose,
            use_checker,
        }
    }
    
    /// 使用 checker 验证输出
    fn validate_with_checker(&self, input: &str, output: &str, scheduler_name: &str) -> bool {
        if !self.use_checker {
            return true; // 没有 checker 时默认通过
        }
        
        let checker_path = Path::new("checker/checker");
        let tmp = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        
        let in_path = tmp.join(format!("gmp_check_in_{}_{}.txt", scheduler_name, timestamp));
        let out_path = tmp.join(format!("gmp_check_out_{}_{}.txt", scheduler_name, timestamp));
        let ans_path = tmp.join(format!("gmp_check_ans_{}_{}.txt", scheduler_name, timestamp));
        
        // 写入临时文件
        if std::fs::write(&in_path, input).is_err() {
            if self.verbose {
                eprintln!("  ⚠ 无法写入临时输入文件");
            }
            return false;
        }
        
        if std::fs::write(&out_path, output).is_err() {
            if self.verbose {
                eprintln!("  ⚠ 无法写入临时输出文件");
            }
            return false;
        }
        
        if std::fs::write(&ans_path, "dummy").is_err() {
            if self.verbose {
                eprintln!("  ⚠ 无法写入临时答案文件");
            }
            return false;
        }
        
        // 执行 checker
        let result = Command::new(checker_path)
            .arg(&in_path)
            .arg(&out_path)
            .arg(&ans_path)
            .output();
        
        // 清理临时文件
        let _ = std::fs::remove_file(&in_path);
        let _ = std::fs::remove_file(&out_path);
        let _ = std::fs::remove_file(&ans_path);
        
        match result {
            Ok(output) => {
                let success = output.status.success();
                if self.verbose {
                    if success {
                        eprintln!("  ✓ Checker 验证通过");
                    } else {
                        eprintln!("  ✗ Checker 验证失败");
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if !stderr.is_empty() {
                            eprintln!("    错误: {}", stderr.trim());
                        }
                    }
                }
                success
            }
            Err(e) => {
                if self.verbose {
                    eprintln!("  ⚠ 无法执行 checker: {}", e);
                }
                false
            }
        }
    }
    
    /// 主入口：调度所有策略并返回最优结果
    pub fn dispatch(&self, input: &str) -> Result<String, String> {
        // 1. 解析输入（只解析一次）
        let (l, m, reqs) = parse_input(input)?;
        
        if self.verbose {
            eprintln!("=== Dispatcher 启动 ===");
            eprintln!("参数: L={}, M={}, N={}", l, m, reqs.len());
            eprintln!("注册的调度器数量: {}", self.registry.get_schedulers().len());
        }
        
        // 2. 依次运行所有调度器
        let mut results: Vec<ScheduleResult> = Vec::new();
        
        for scheduler in self.registry.get_schedulers() {
            let name = scheduler.name().to_string();
            
            if self.verbose {
                eprintln!("\n--- 运行: {} ---", name);
            }
            
            let result = match scheduler.schedule(&reqs, l, m) {
                Ok(output) => {
                    let mut result = ScheduleResult::from_output(name.clone(), output.clone());
                    
                    if self.verbose {
                        eprintln!("格式检查: {}", if result.is_valid { "✓ 合法" } else { "✗ 非法" });
                        eprintln!("Fin 时间: {}", result.fin_time);
                        if let Some(ref err) = result.error_msg {
                            eprintln!("格式错误: {}", err);
                        }
                    }
                    
                    // 如果格式合法，进一步用 checker 验证
                    if result.is_valid {
                        let checker_ok = self.validate_with_checker(input, &output, &name);
                        if !checker_ok {
                            result.is_valid = false;
                            result.error_msg = Some("Checker 验证失败".to_string());
                        }
                    }
                    
                    result
                }
                Err(err) => {
                    if self.verbose {
                        eprintln!("状态: ✗ 执行失败");
                        eprintln!("错误: {}", err);
                    }
                    ScheduleResult::from_error(name, err)
                }
            };
            
            results.push(result);
        }
        
        // 3. 选择最优结果
        if self.verbose {
            eprintln!("\n=== 选择最优结果 ===");
        }
        
        match self.selector.select_best(&results) {
            Some(best) => {
                if self.verbose {
                    eprintln!("最优策略: {}", best.scheduler_name);
                    eprintln!("Fin 时间: {}", best.fin_time);
                }
                Ok(best.output.clone())
            }
            None => {
                if self.verbose {
                    eprintln!("✗ 没有找到合法的结果");
                }
                Err("所有调度器都失败了".to_string())
            }
        }
    }
}

/// 便捷函数：保持与原 solve 函数的兼容性
pub fn solve(input: &str) -> String {
    let dispatcher = Dispatcher::new();
    dispatcher.dispatch(input).unwrap_or_else(|err| {
        eprintln!("调度失败: {}", err);
        format!("Fin -1")
    })
}
