//! WebGAL 资源

use std::path::{Path, PathBuf};

use strum_macros::{AsRefStr, Display};

use crate::traits::asset::Asset;

/// WebGAL 资源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, Display)]
#[strum(serialize_all = "camelCase")]
pub enum ResourceType {
    Background,
    Bgm,
    Vocal,
    Figure,
}

/// WebGAL 资源
///
/// 作为 Resolver 的解析结果, Downloader 的接收类型.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Resource {
    pub kind: ResourceType,
    pub url: String,
    pub path: String,
}

impl Asset for Resource {
    fn relative_path(&self) -> String {
        match self.kind {
            ResourceType::Figure => super::default_model_config_path(&self.path),
            _ => self.path.clone(),
        }
    }

    fn absolute_path(&self, root: impl AsRef<Path>) -> PathBuf {
        root.as_ref().join(format!("{}/{}", self.kind, self.path))
    }
}
