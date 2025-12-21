//! 脚本转译

use std::sync::Arc;

use crate::{
    error::*,
    models::{
        bestdori,
        webgal::{self, Resource},
    },
};

/// 转译结果
pub struct TranspileResult {
    pub story: webgal::Story,
    pub resources: Vec<Arc<Resource>>,
    pub errors: Vec<Error>,
}

/// 脚本转译器
///
/// 转译器应该内部持有 Resolver.
///
/// 注意到转译过程不会发生致命错误, 则应只跳过有问题的部分.
pub trait Transpile {
    /// 转译脚本
    ///
    /// 接收 Bestdori 脚本, 返回 WebGAL 脚本 + 资源, 以及收集到的错误.
    fn transpile(self, story: &bestdori::Story) -> TranspileResult;
}
