//! WebGAL 故事脚本

use crate::traits::asset::Asset;

use super::Action;

/// WebGAL 故事脚本
pub struct Story(Vec<Scene>);

/// WebGAL 故事场景
pub struct Scene {
    pub path: String,
    pub actions: Vec<Action>,
}

impl Asset for Scene {
    fn relative_path(&self) -> String {
        format!("scene/{}", self.path)
    }
}
