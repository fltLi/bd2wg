//! bd2wg 错误处理

/// bd2wg 标准返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// bd2wg 标准错误类型
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("serde failed to parse json: {0}")]
    SerdeJson(#[from] serde_json::Error),
}
