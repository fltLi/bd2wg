//! WebGAL 故事脚本

use crate::{models::webgal::CallSceneAction, traits::asset::Asset};

use super::Action;

const START_SCENE_PATH: &str = "start.txt";

/// WebGAL 故事脚本
pub struct Story(pub Vec<Scene>);

/// WebGAL 故事场景
#[derive(Default)]
pub struct Scene {
    pub path: String,
    pub actions: Vec<Action>,
}

impl Scene {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            ..Self::default()
        }
    }

    /// 生成初始场景
    pub fn new_start_scene() -> Self {
        Self {
            path: START_SCENE_PATH.to_string(),
            actions: vec![
                CallSceneAction {
                    file: START_SCENE_PATH.to_string(),
                }
                .into(),
            ],
        }
    }
}

impl Asset for Scene {
    fn relative_path(&self) -> String {
        format!("scene/{}", self.path)
    }
}
