//! bd2wg 业务层

#![allow(dead_code, unused, clippy::too_many_arguments)]
#![feature(file_buffered, impl_trait_in_bindings, trim_prefix_suffix)]

pub mod models;
pub mod pipeline;

pub mod constant {
    //! bd2wg 常量定义

    pub const DOWNLOAD_HEADER: &str = "./assets/header.json";
    pub const RESOLVE_CONFIG: &str = "./assets/bestdoli.json";
    pub const WEBGAL_START_SCENE: &str = "start.txt";
    pub const WEBGAL_LIVE2D_VERSION: &str = "Sample 1.0.0";
    pub const DOWNLOAD_TASK_LIMIT: usize = 32;
    pub const DOWNLOAD_TIMEOUT_SECS: u64 = 24;
}

pub mod error {
    //! bd2wg 错误类型

    use std::fmt;

    use crate::models::bestdoli::Address;

    /// 通用返回类型
    pub type Result<T> = std::result::Result<T, Error>;

    /// 通用错误类型
    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("Serde failed to parse json: {0}")]
        Json(#[from] serde_json::Error),
        #[error("I/O error: {0}")]
        Io(#[from] std::io::Error),
        #[error("Script error: {0}")]
        Script(#[from] ScriptError),
        #[error("Resolve error: {0}")]
        Resolve(#[from] ResolveError),
        #[error("Download error: {0}")]
        Download(#[from] DownloadError),
        #[error("Pipeline error: {0}")]
        Pipeline(#[from] PipelineError),
    }

    /// 资源解析错误
    #[derive(Debug, thiserror::Error)]
    pub enum ResolveError {
        #[error("Common resource: address: {addr}, kind: {kind}")]
        Common {
            kind: ResolveCommonKind,
            addr: Address,
        },
        #[error("Live2D resource: character {character}, kind: {kind}, attribute: {attr}")]
        Live2D {
            kind: ResolveLive2DKind,
            character: u8,
            attr: String,
        },
    }

    #[derive(Debug, strum::EnumString, strum::Display)]
    pub enum ResolveCommonKind {
        Bgm,
        Se,
        Background,
        CardStill,
    }

    #[derive(Debug, strum::EnumString, strum::Display)]
    pub enum ResolveLive2DKind {
        Model,
        Motion,
        Expression,
    }

    /// 下载相关错误的具体种类（不包含上下文）
    #[derive(Debug, thiserror::Error)]
    pub enum DownloadErrorKind {
        #[error("HTTP status error: {0}")]
        HttpStatus(reqwest::StatusCode),
        #[error("Reqwest error: {0}")]
        Reqwest(#[from] reqwest::Error),
        #[error("I/O error while writing file: {0}")]
        Io(#[from] std::io::Error),
        #[error("Missing URL for resource")]
        UrlMissing,
        #[error("Failed to send task to downloader: {0}")]
        SendError(String),
        #[error("Worker thread panic or join failure")]
        WorkerPanic,
        #[error("Operation timed out")]
        Timeout,
        #[error("Unexpected error: {0}")]
        Unexpected(String),
    }

    /// 带有上下文的下载错误结构体
    #[derive(Debug, thiserror::Error)]
    pub struct DownloadError {
        #[source]
        pub kind: DownloadErrorKind,
        pub url: Option<String>,
        pub path: Option<String>,
    }

    impl fmt::Display for DownloadError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut context = String::new();
            if let Some(url) = &self.url {
                context += &format!(", url: {url}");
            }
            if let Some(path) = &self.path {
                context += &format!(", path: {path}");
            }
            write!(f, "kind: {}{}", self.kind, context)
        }
    }

    impl From<DownloadErrorKind> for DownloadError {
        fn from(kind: DownloadErrorKind) -> Self {
            DownloadError {
                kind,
                url: None,
                path: None,
            }
        }
    }

    impl From<reqwest::Error> for DownloadError {
        fn from(err: reqwest::Error) -> Self {
            DownloadError::from(DownloadErrorKind::from(err))
        }
    }

    impl From<std::io::Error> for DownloadError {
        fn from(err: std::io::Error) -> Self {
            DownloadError::from(DownloadErrorKind::from(err))
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum ScriptError {
        #[error("Unknown action")]
        Unknown,
        #[error("Try access unexisted id: {0}")]
        IdNotFound(u8),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum PipelineError {
        #[error("Start when not available.")]
        BadStart,
        #[error("Something paniced. :(")]
        Paniced,
    }
}
