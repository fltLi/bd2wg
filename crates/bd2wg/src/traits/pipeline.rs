//! 工作管线

use crate::error::*;

use super::handle::Handle;

/// 转译状态
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranspileState {
    pub scene: usize,
    pub action: usize,
}

/// 转译结果
#[derive(Debug, Default)]
pub struct TranspileResult {
    pub state: TranspileState,
    pub errors: Vec<Error>,
}

/// 下载状态
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadState {
    pub done: usize,
    pub total: usize,
}

/// 下载结果
#[derive(Debug, Default)]
pub struct DownloadResult {
    pub state: DownloadState,
    pub errors: Vec<Error>,
}

/// 转译管线
///
/// 非阻塞运行, 转移脚本并写入场景文件
pub trait TranspilePipeline:
    Handle<Result = (TranspileResult, Result<Box<dyn DownloadPipeline>>)>
{
    fn state(&self) -> TranspileState;
}

/// 下载管线
///
/// 非阻塞运行, 下载所需的资源
pub trait DownloadPipeline: Handle<Result = DownloadResult> {
    fn state(&self) -> DownloadState;
}

/// 阻塞执行转译
pub fn run_pipeline_blocking(
    pipe: Box<dyn TranspilePipeline>,
) -> (TranspileResult, Result<DownloadResult>) {
    let (trans_res, pipe) = pipe.join();
    (trans_res, pipe.map(|pipe| pipe.join()))
}
