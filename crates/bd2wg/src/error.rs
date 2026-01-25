//! bd2wg 错误处理

use std::{io, path::PathBuf};

use thiserror::Error;

use crate::{models::bestdori, traits::resolve::ResourceType};

/// bd2wg 返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// bd2wg 错误类型
#[derive(Debug, Error)]
pub enum Error {
    #[error("File operation failed: {0}")]
    File(#[from] FileError),

    #[error("Download failed: {0}")]
    Download(#[from] DownloadError),

    #[error("Transpile failed: {0}")]
    Transpile(#[from] TranspileError),
}

/// 文件操作错误
///
/// 读取并解析 Bestdori 脚本, 写入 WebGAL 脚本时发生.
#[derive(Debug, Error)]
pub enum FileError {
    #[error("JSON parse error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("File I/O error: {0}")]
    Io(#[from] io::Error),
}

/// 下载错误
#[derive(Debug, Error)]
#[error("Download failed: {url} -> {path:?}: {error}")]
pub struct DownloadError {
    pub url: String,
    pub path: PathBuf,
    #[source]
    pub error: DownloadErrorKind,
}

impl DownloadError {
    /// 创建带上下文的错误
    pub fn with_context(
        url: impl Into<String>,
        path: impl Into<PathBuf>,
        err: DownloadErrorKind,
    ) -> Self {
        Self {
            url: url.into(),
            path: path.into(),
            error: err,
        }
    }

    /// 创建不带上下文的错误
    pub fn without_context(err: DownloadErrorKind) -> Self {
        Self {
            url: String::new(),
            path: PathBuf::new(),
            error: err,
        }
    }
}

impl From<DownloadErrorKind> for DownloadError {
    /// 将没有上下文的 DownloadErrorKind 包装为 DownloadError
    fn from(value: DownloadErrorKind) -> Self {
        DownloadError::without_context(value)
    }
}

#[derive(Debug, Error)]
pub enum DownloadErrorKind {
    #[error("Network request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("File write failed: {0}")]
    Io(#[from] io::Error),
}

/// 重定向错误
#[derive(Debug, Error)]
pub enum RedirectError {

}

/// 解析错误
#[derive(Debug, Error)]
#[error("Unable to resolve resource: kind={kind:?}, resource={resource:?}")]
pub struct ResolveError {
    pub kind: ResourceType,
    pub resource: bestdori::Resource,
}

/// 转译错误
#[derive(Debug, Error)]
#[error("Transpile failed: {error}, in {scene} - line {line}, action={action:?}")]
pub struct TranspileError {
    pub scene: String,
    pub line: usize,
    pub action: Box<bestdori::Action>,
    #[source]
    pub error: TranspileErrorKind,
}

#[derive(Debug, Error)]
pub enum TranspileErrorKind {
    #[error("Unknown command")]
    Unknown,

    #[error("Uninitialized figure model called: {0}")]
    UninitFigure(u8),

    #[error("Resource redirect failed: {0}")]
    Redirect(#[from] RedirectError),

    #[error("Resource resolve failed: {0}")]
    Resolve(#[from] ResolveError),
}
