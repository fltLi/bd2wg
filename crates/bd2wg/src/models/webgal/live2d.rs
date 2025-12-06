//! WebGAL Live2D 配置

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use serde_with::{Map, serde_as};

use crate::models::bestdori;

/// WebGAL Live2D 版本
pub const WEBGAL_LIVE2D_VERSION: &str = "Sample 1.0.0";
pub const WEBGAL_LIVE2D_CONFIG: &str = "model.json";

pub const WEBGAL_LIVE2D_MODEL: &str = "model.moc";
pub const WEBGAL_LIVE2D_PHYSICS: &str = "physics.json";
pub const WEBGAL_LIVE2D_TEXTURES: &str = "textures/";

pub const WEBGAL_LIVE2D_MOTIONS: &str = "motions/";
pub const WEBGAL_LIVE2D_EXPRESSIONS: &str = "expressions/";

/// 从模型路径生成默认模型路径
pub fn default_model_config_path(root: &str) -> String {
    format!("{root}{WEBGAL_LIVE2D_CONFIG}")
}

/// WebGAL Live2D 配置文件
#[serde_as]
#[derive(Debug, Clone, Builder, Deserialize, Serialize)]
pub struct Model {
    pub version: String,
    pub layout: Layout,
    #[serde(rename = "hit_areas_custom")]
    pub hit_areas: HitAreas,
    pub model: String,
    pub physics: String,
    pub textures: Vec<String>,
    #[serde_as(as = "Map<_, _>")]
    pub motions: Vec<(String, Vec<Motion>)>,
    pub expressions: Vec<Expression>,
}

impl Model {
    /// 解析 Bestdori Live2D BuildScript, 获取配置和资源 (url / relative path)
    pub fn from_bestdori_model(model: bestdori::Model) -> (Self, Vec<(String, PathBuf)>) {
        let mut res = Vec::with_capacity(
            1 + model.textures.len() + model.motions.len() + model.expessions.len(),
        );

        // 模型和物理采用默认路径
        res.push((model.model.url(), WEBGAL_LIVE2D_MODEL.into()));
        res.push((model.physics.url(), WEBGAL_LIVE2D_PHYSICS.into()));

        // 解析纹理, 动作和表情
        let model = ModelBuilder::default()
            .textures(
                model
                    .textures
                    .iter()
                    .map(|url| {
                        let path = format!("{WEBGAL_LIVE2D_TEXTURES}{}", url.path());

                        res.push((url.url(), PathBuf::from(&path)));
                        path
                    })
                    .collect(),
            )
            .motions(
                model
                    .motions
                    .iter()
                    .map(|url| {
                        let file = url.file.strip_suffix(".mtn.bytes").unwrap_or(&url.file);
                        let path = format!("{WEBGAL_LIVE2D_MOTIONS}{file}.mtn");

                        res.push((url.url(), PathBuf::from(&path)));
                        (file.to_string(), vec![file.to_string().into()])
                    })
                    .collect(),
            )
            .expressions(
                model
                    .expessions
                    .iter()
                    .map(|url| {
                        let file = url.file.strip_suffix(".exp.json").unwrap_or(&url.file);
                        let path = format!("{WEBGAL_LIVE2D_EXPRESSIONS}{}", url.file);

                        res.push((url.url(), PathBuf::from(&path)));
                        Expression {
                            name: file.to_string(),
                            file: path.to_string(),
                        }
                    })
                    .collect(),
            )
            .build()
            .unwrap();

        (model, res)
    }
}

impl Default for Model {
    fn default() -> Self {
        Self {
            version: WEBGAL_LIVE2D_VERSION.to_string(),
            layout: Layout::default(),
            hit_areas: HitAreas::default(),
            model: WEBGAL_LIVE2D_MODEL.to_string(),
            physics: WEBGAL_LIVE2D_PHYSICS.to_string(),
            textures: Vec::default(),
            motions: Vec::default(),
            expressions: Vec::default(),
        }
    }
}

#[derive(Debug, Clone, Builder, Deserialize, Serialize)]
pub struct Layout {
    #[serde(rename = "center_x")]
    pub x: i16,
    #[serde(rename = "center_y")]
    pub y: i16,
    pub width: i16,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 2,
        }
    }
}

#[derive(Debug, Clone, Builder, Deserialize, Serialize)]
pub struct HitAreas {
    pub head_x: (f32, f32),
    pub head_y: (f32, f32),
    pub body_x: (f32, f32),
    pub body_y: (f32, f32),
}

impl Default for HitAreas {
    fn default() -> Self {
        Self {
            head_x: (-0.25, 1.),
            head_y: (0.25, 0.2),
            body_x: (-0.3, 0.2),
            body_y: (0.3, -1.9),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Motion {
    pub file: String,
}

impl From<String> for Motion {
    fn from(value: String) -> Self {
        Self { file: value }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Expression {
    pub name: String,
    pub file: String,
}
