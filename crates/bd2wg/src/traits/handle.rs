//! 任务句柄

/// 任务句柄
pub trait Handle {
    type Result;

    /// 等待结束并获取运行结果
    fn join(self: Box<Self>) -> Self::Result;

    /// 中断执行
    ///
    /// 建议此类型的实现在 Drop 中采用 cancel 方法
    fn cancel(&mut self);

    /// 是否结束
    fn is_finished(&self) -> bool;
}
