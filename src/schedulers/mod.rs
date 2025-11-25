/// 调度器实现模块
/// 
/// 要添加新的调度策略，只需：
/// 1. 创建新文件实现 Scheduler trait
/// 2. 在此模块中 pub mod 导出
/// 3. 在 dispatcher.rs 中注册

pub mod greedy_scheduler;
pub mod base_scheduler;
pub mod lru_scheduler;

// 重导出，方便使用
pub use greedy_scheduler::GreedyScheduler;
pub use base_scheduler::BaseScheduler;
pub use lru_scheduler::LruScheduler;
