//! live2d 配置

use std::collections::HashMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_with::{Map, serde_as};

use crate::constant::*;
use crate::error::*;

/// bestdori live2d 配置
///
/// 数据包下 buildData.asset 文件  
/// 使用关联方法 from_bytes 反序列化
#[derive(Deserialize)]
pub struct ModelBundle {
    pub model: Bundle,
    pub physics: Bundle,
    pub textures: Vec<Bundle>,
}

#[derive(Deserialize)]
struct ModelBundleHelper {
    #[serde(rename = "Base")]
    model: ModelBundle,
}

impl ModelBundle {
    pub fn from_bytes(value: &[u8]) -> Result<Self> {
        let helper: ModelBundleHelper = serde_json::from_slice(value)?;
        let mut model = helper.model;
        model.model.file = String::from(model.model.file.trim_suffix(".bytes"));
        Ok(model)
    }
}

#[derive(Deserialize)]
pub struct Bundle {
    #[serde(rename = "bundleName")]
    pub bundle: String,
    #[serde(rename = "fileName")]
    pub file: String,
}

/// webgal live2d 配置
#[serde_as]
#[derive(Serialize)]
pub struct ModelConfig {
    pub version: String,
    pub layout: Layout,
    #[serde(rename = "hit_areas_custom")]
    pub hit: HitArea,
    #[serde(flatten)]
    pub model: Model,
    #[serde_as(as = "Rc<Map<_, _>>")]
    pub motions: Rc<Vec<(String, Vec<Motion>)>>, // 注意到数据包中的动作每组只有一个
    pub expressions: Rc<Vec<Expression>>,
}

impl ModelConfig {
    pub fn new(
        model: Model,
        motions: Rc<Vec<(String, Vec<Motion>)>>,
        expressions: Rc<Vec<Expression>>,
    ) -> Self {
        Self {
            version: String::from(WEBGAL_LIVE2D_VERSION),
            layout: Layout::default(),
            hit: HitArea::default(),
            model,
            motions,
            expressions,
        }
    }
}

#[derive(Serialize)]
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

#[derive(Serialize)]
pub struct HitArea {
    pub head_x: (f32, f32),
    pub head_y: (f32, f32),
    pub body_x: (f32, f32),
    pub body_y: (f32, f32),
}

impl Default for HitArea {
    fn default() -> Self {
        Self {
            head_x: (-0.25, 1.),
            head_y: (0.25, 0.2),
            body_x: (-0.3, 0.2),
            body_y: (0.3, -1.9),
        }
    }
}

/// 模型 (衣装) 基本信息
#[derive(Clone, Serialize)]
pub struct Model {
    pub model: String,
    pub physics: String,
    pub textures: Vec<String>,
}

#[derive(Serialize)]
pub struct Motion {
    pub file: String,
}

impl From<Motion> for Vec<Motion> {
    fn from(val: Motion) -> Self {
        vec![val]
    }
}

#[derive(Serialize)]
pub struct Expression {
    pub name: String,
    pub file: String,
}

#[test]
fn test_model_bundle() {
    let text = r#"{
            "Base": {
                "model": {
                    "bundleName": "live2d/chara/039_casual-2023",
                    "fileName": "soyo_casual-2023.moc.bytes"
                },
                "physics": {
                    "bundleName": "live2d/chara/039_casual-2023",
                    "fileName": "soyo_casual-2023.physics.json"
                },
                "textures": [
                    {
                        "bundleName": "live2d/chara/039_general",
                        "fileName": "texture_00.png"
                    },
                    {
                        "bundleName": "live2d/chara/039_casual-2023",
                        "fileName": "texture_01.png"
                    }
                ]
            }
        }"#;
    let model = ModelBundle::from_bytes(text.as_bytes()).unwrap();
}
