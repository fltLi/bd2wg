//! Bestdori 资源下载

use crate::error::*;
use crate::models::webgal::Resource;
use crate::traits::handle::Handle;

/// Bestdori 资源下载器
///
/// 根据 WebGAL 资源类型下载 Bestdori 资源到指定路径.
///
/// 下载器返回任务句柄, 独立地管理每个资源的下载.
/// 同时, 下载器也是管理所有任务的生命周期的任务句柄.
///
/// 建议下载器内部管理基础下载任务池, 接受每个任务句柄的调用.
pub trait Downloader: Handle<Result = ()> {
    /// 启动下载任务
    fn download<R: AsRef<Resource>>(&mut self, resource: R) -> Result<Box<dyn Handle<Result = Result<()>>>>;
}
