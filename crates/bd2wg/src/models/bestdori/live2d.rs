//! Bestdori Live2D 配置

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::*;

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
    file: String,
    #[serde(rename = "bundleName")]
    bundle: String,
}

/// Live2D 配置文件
///
/// 请使用 Self::from_str 方法经由中间结构体反序列化.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Model {
    pub model: Live2dPath,
    pub physics: Live2dPath,
    pub textures: Vec<Live2dPath>,
    pub motions: Vec<Live2dPath>,
    pub expessions: Vec<Live2dPath>,
}

impl FromStr for Model {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let helper: ModelHelper = serde_json::from_str(s)?;
        Ok(helper.model)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ModelHelper {
    #[serde(rename = "Base")]
    model: Model,
}
