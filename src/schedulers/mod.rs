/// 调度器实现模块
/// 
/// 要添加新的调度策略，只需：
/// 1. 创建新文件实现 Scheduler trait
/// 2. 在此模块中 pub mod 导出
/// 3. 在 dispatcher.rs 中注册

pub mod greedy_scheduler;
pub mod base_scheduler;
pub mod lru_scheduler;
pub mod aggressive_prefetch_scheduler;
pub mod lazy_scheduler;
pub mod wavefront_scheduler;
pub mod slidingwindowgreedy_scheduler;
pub mod overlap_aware_eviction_scheduler;

pub mod cost_benefit_scheduler;
pub mod lfu_scheduler;
pub mod adaptive_scheduler;
pub mod fragmentation_scheduler;
pub mod just_in_time_scheduler;
pub mod generational_scheduler;

// 重导出，方便使用
pub use greedy_scheduler::GreedyScheduler;
pub use base_scheduler::BaseScheduler;
pub use lru_scheduler::LruScheduler;
pub use aggressive_prefetch_scheduler::AggressivePrefetchScheduler;
pub use lazy_scheduler::LazyScheduler;
pub use wavefront_scheduler::WavefrontScheduler;
pub use slidingwindowgreedy_scheduler::SlidingWindowGreedyScheduler;
pub use overlap_aware_eviction_scheduler::OverlapAwareEvictionScheduler;
pub use cost_benefit_scheduler::CostBenefitScheduler;
pub use lfu_scheduler::LfuScheduler;
pub use adaptive_scheduler::AdaptiveScheduler;
pub use fragmentation_scheduler::FragmentationScheduler;
pub use just_in_time_scheduler::JustInTimeScheduler;
pub use generational_scheduler::GenerationalScheduler;