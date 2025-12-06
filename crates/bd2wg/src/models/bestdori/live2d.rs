//! Bestdori Live2D 配置

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::*;

use super::*;

/// Live2D 动作
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Motion {
    pub delay: f32,
    pub character: u8, // *Bushiroad 的生产力没有超过 u8
    pub motion: String,
    pub expression: String,
}

/// Live2D 资源路径
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Live2dPath {
    #[serde(rename = "fileName")]
    pub file: String,
    #[serde(rename = "bundleName")]
    pub bundle: String,
}

impl Live2dPath {
    /// 连接 bundle 与 file
    pub fn path(&self) -> String {
        format!("{}_rip/{}", self.bundle, self.file)
    }

    /// Bestdori 资源链接
    pub fn url(&self) -> String {
        format!("{BESTDORI_ASSETS_URL_ROOT}{}", self.path())
    }
}

/// Bestdori Live2D 配置文件
///
/// 请使用 Self::from_slice 方法经由中间结构体反序列化.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Model {
    pub model: Live2dPath,
    pub physics: Live2dPath,
    pub textures: Vec<Live2dPath>,
    pub motions: Vec<Live2dPath>,
    pub expessions: Vec<Live2dPath>,
}

impl Model {
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let helper: ModelHelper = serde_json::from_slice(bytes)?;
        Ok(helper.into())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ModelHelper {
    #[serde(rename = "Base")]
    model: Model,
}

impl From<ModelHelper> for Model {
    fn from(value: ModelHelper) -> Self {
        value.model
    }
}
