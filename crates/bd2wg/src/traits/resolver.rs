//! 资源解析

use std::{ops::Deref, sync::Arc};

use crate::error::*;
use crate::impl_deref_for_asref;
use crate::models::{bestdori, webgal};

/// 资源解析类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Image,
    Bgm,
    Se,
}

/// 资源解析结果
pub enum ResourceEntry {
    Vacant(Arc<webgal::Resource>),
    Occupied(*const webgal::Resource),
}

impl ResourceEntry {
    /// 是否为新值
    pub fn is_vacant(&self) -> bool {
        matches!(self, Self::Vacant(_))
    }
}

impl AsRef<webgal::Resource> for ResourceEntry {
    fn as_ref(&self) -> &webgal::Resource {
        match self {
            Self::Vacant(v) => v.as_ref(),
            Self::Occupied(o) => unsafe { o.as_ref().unwrap() },
        }
    }
}

impl_deref_for_asref! {ResourceEntry, webgal::Resource}

/// 资源解析器
///
/// 解析 Bestdori 资源为 WebGAL 资源 + 下载链接.
///
/// 解析器会自动去重, 避免重复资源下载.
pub trait Resolver {
    /// 解析常规资源
    fn resolve_normal(
        &mut self,
        res: &bestdori::Resource,
        kind: ResourceType,
    ) -> Result<ResourceEntry>;

    /// 解析 Live2D 资源
    fn resolve_model(&mut self, costume: &str) -> ResourceEntry;
}
