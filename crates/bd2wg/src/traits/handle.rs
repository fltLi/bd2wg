//! 任务句柄

use std::ops::Deref;

use crate::{error::*, traits::handle};

/// 任务句柄
pub trait Handle {
    type Result;

    /// 等待结束并获取运行结果
    fn join(self) -> Result<Self::Result>;

    /// 中断执行
    ///
    /// 建议此类型的实现在 Drop 中采用 shutdown 方法
    fn shutdown(self) -> Result<()>;

    /// 是否结束
    fn is_finished(&self) -> bool;
}

/// 为 Handle 实现 shutdown Drop
macro_rules! impl_drop_for_handle {
    ($t:ty) => {
        impl Drop for $t {
            fn drop(&mut self) {
                if !self.is_finished() {
                    let _ = self.shutdown();
                }
            }
        }
    };
}
