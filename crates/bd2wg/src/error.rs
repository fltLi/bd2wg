//! bd2wg 错误处理

use std::{io, path::PathBuf};

use thiserror::Error;

use crate::models::bestdori;
use crate::traits::resolve::ResourceType;

/// bd2wg 标准返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// bd2wg 标准错误类型
#[derive(Debug, Error)]
pub enum Error {
    #[error("json 处理失败: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("下载失败: {0}")]
    Download(#[from] DownloadError),

    #[error("转译失败: {0}")]
    Transpile(#[from] TranspileError),
}

/// 下载错误
#[derive(Debug, Error)]
#[error("下载失败: {url} -> {path:?}: {error}")]
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
    #[error("网络请求失败: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("文件写入失败: {0}")]
    Io(#[from] io::Error),
}

/// 资源解析错误
#[derive(Debug, Error)]
#[error("无法解析资源: kind={kind:?}, resource={resource:?}")]
pub struct ResolveError {
    pub kind: ResourceType,
    pub resource: bestdori::Resource,
}

/// 转译错误
#[derive(Debug, Error)]
#[error("转译失败: {error}, action={action:?}")]
pub struct TranspileError {
    pub action: Box<bestdori::Action>,
    #[source]
    pub error: TranspileErrorKind,
}

#[derive(Debug, Error)]
pub enum TranspileErrorKind {
    #[error("未知指令")]
    Unknown,

    #[error("调用未入场的人物模型: {0}")]
    UninitFigure(u8),

    #[error("资源解析失败: {0}")]
    Resolve(#[from] ResolveError),
}
