//! Bestdori 下载器
//!
//! 下载器由一个基础且通用的 DownloadPool 和针对 Bestdori 资源类型的上层封装实现.

mod pool;
mod service;

pub use service::Downloader;
