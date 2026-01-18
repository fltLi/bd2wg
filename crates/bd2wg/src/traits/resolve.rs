//! 资源解析

use std::{ops::Deref, sync::Arc};

use crate::{
    error::ResolveError,
    impl_deref_for_asref,
    models::{bestdori, webgal},
};

pub type ResolveResult<T> = Result<T, ResolveError>;

/// 常规资源解析类型
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

/// 具体模型展示解析
pub trait ModelDisplayResolve {
    fn resolve_motion(&self, motion: &str) -> ResolveResult<String>;

    fn resolve_expression(&self, expression: &str) -> ResolveResult<String>;
}

// /// 整理模型展示解析器生成结果
// pub fn map_model_display_resolver_resolt<R, E>(
//     res: Option<ResolveResult<R>>,
// ) -> (Option<R>, Result<(), E>)
// where
//     R: ModelDisplayResolve,
//     E: From<ResolveError>,
// {
//     match res {
//         None => (None, Ok(())),
//         Some(Ok(res)) => (Some(res), Ok(())),
//         Some(Err(err)) => (None, Err(err.into())),
//     }
// }

/// 资源解析器
///
/// 解析 Bestdori 资源为 WebGAL 资源 + 下载链接.
///
/// 解析器会自动去重, 避免重复资源下载.
///
/// 对于 Live2D 模型, 可能会启用本地复用, 并返回表情 / 动作转换器.
pub trait Resolve {
    type ModelDisplayResolver: ModelDisplayResolve;

    /// 解析常规资源
    fn resolve_normal(
        &mut self,
        res: &bestdori::Resource,
        kind: ResourceType,
    ) -> ResolveResult<ResourceEntry>;

    /// 解析 Live2D 资源
    fn resolve_model(
        &mut self,
        costume: &str,
    ) -> (ResourceEntry, Option<Self::ModelDisplayResolver>);
}
