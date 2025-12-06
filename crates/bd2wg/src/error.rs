//! bd2wg 错误处理

use std::{io, path::PathBuf};

use thiserror::Error;

use crate::models::{bestdori::ResourceType, webgal::Resource};

/// bd2wg 标准返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// bd2wg 标准错误类型
#[derive(Debug, Error)]
pub enum Error {
    #[error("serde_json failed: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("download failed: {0}")]
    Download(#[from] DownloadError),
}

/// 下载错误
#[derive(Debug, Error)]
#[error("download error: url: {}, path: {:?}, error: {}", self.url, self.path, self.error)]
pub struct DownloadError {
    pub url: String,
    pub path: PathBuf,
    pub error: DownloadErrorKind,
}

impl From<DownloadErrorKind> for DownloadError {
    /// 创建不包含 url 和 path 的下载错误
    fn from(value: DownloadErrorKind) -> Self {
        Self {
            url: String::default(),
            path: PathBuf::default(),
            error: value,
        }
    }
}

#[derive(Debug, Error)]
pub enum DownloadErrorKind {
    #[error("reqwest failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("io failed: {0}")]
    Io(#[from] io::Error),
}
