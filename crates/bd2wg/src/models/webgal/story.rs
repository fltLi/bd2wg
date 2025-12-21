//! WebGAL 故事脚本

use std::fmt::{self, Display};

use crate::{impl_iter_for_tuple, models::webgal::display_action_iter, traits::asset::Asset};

use super::Action;

const START_SCENE_PATH: &str = "start.txt";

/// WebGAL 故事脚本
pub struct Story(pub Vec<Scene>);

impl Story {
    /// 获取场景数和指令数
    pub fn len(&self) -> (usize, usize) {
        (
            self.0.len(),
            self.iter().map(|scene| scene.actions.len()).sum(),
        )
    }
}

impl_iter_for_tuple! {Story, Scene}

impl Display for Scene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_action_iter(self.actions.iter(), f)
    }
}

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
        Self::new(START_SCENE_PATH)
    }
}

impl Asset for Scene {
    fn relative_path(&self) -> String {
        format!("scene/{}", self.path)
    }
}
