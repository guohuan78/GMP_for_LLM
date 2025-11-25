// 库文件：导出公共接口，方便测试和外部使用

pub mod memory;
pub mod types;
pub mod scheduler_trait;
pub mod schedulers;
pub mod dispatcher;

// 重导出常用类型
pub use types::{Req, parse_input};
pub use scheduler_trait::{Scheduler, ScheduleResult};
pub use dispatcher::{Dispatcher, solve};
