//! WebGAL 资源

use std::path::{Path, PathBuf};

use strum_macros::{AsRefStr, Display, EnumString};

use crate::traits::asset::Asset;

/// WebGAL 资源类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr, Display)]
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

impl Resource {
    fn default_relative_path(&self) -> String {
        format!("{}/{}", self.kind, self.path)
    }
}

impl Asset for Resource {
    fn relative_path(&self) -> String {
        match self.kind {
            ResourceType::Figure => format!("{}/model.json", self.default_relative_path()),
            _ => self.default_relative_path(),
        }
    }

    fn absolute_path<P: AsRef<Path>>(&self, root: P) -> PathBuf {
        root.as_ref().join(self.default_relative_path())
    }
}
