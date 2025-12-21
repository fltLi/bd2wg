//! 资源解析器

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
};

use crate::{
    error::*,
    models::{
        bestdori::{
            self, BESTDORI_ASSET_URL_MODEL, BESTDORI_ASSET_URL_MODEL_BUILDER,
            BESTDORI_ASSET_URL_ROOT, BESTDORI_ASSET_URL_SE,
        },
        webgal,
    },
    traits::resolve::*,
    utils::*,
};

const RESOURCE_IMAGE_EXTEND: &str = ".png";
const RESOURCE_SOUND_EXTEND: &str = ".mp3";

/// 根据 webgal 资源类型获取后缀名
macro_rules! get_extend {
    ($kind:ident) => {
        match $kind {
            webgal::ResourceType::Background => RESOURCE_IMAGE_EXTEND,
            webgal::ResourceType::Bgm | webgal::ResourceType::Vocal => RESOURCE_SOUND_EXTEND,
            _ => return None,
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ResourceKey {
    Normal(bestdori::Resource, ResourceType),
    Model(String),
}

/// 资源解析器
///
/// 解析 Bestdori 资源, 供下载器和转译器使用.
#[derive(Default)]
pub struct Resolver {
    resource: HashMap<ResourceKey, Arc<webgal::Resource>>,
}

impl Resolver {
    /// 创建空的解析器
    pub fn new() -> Self {
        Self::default()
    }

    /// 查找已存在的元素 / 插入
    fn get_or_insert(
        &mut self,
        key: ResourceKey,
        call: impl FnOnce() -> ResolveResult<webgal::Resource>,
    ) -> ResolveResult<ResourceEntry> {
        Ok(match self.resource.entry(key) {
            // 解析并保存, 返回拷贝的指针
            Entry::Vacant(v) => ResourceEntry::Vacant(v.insert(Arc::new(call()?)).clone()),

            // 资源已存在, 返回保存的裸指针
            Entry::Occupied(o) => ResourceEntry::Occupied(Arc::as_ptr(o.get())),
        })
    }

    // ---------------- resolve ----------------

    /// 解析资源
    fn resolve(res: &bestdori::Resource, kind: ResourceType) -> Option<webgal::Resource> {
        match kind {
            ResourceType::Image => Self::resolve_image(res),
            ResourceType::Bgm => Self::resolve_bgm(res),
            ResourceType::Se => Self::resolve_se(res),
        }
    }

    fn resolve_image(res: &bestdori::Resource) -> Option<webgal::Resource> {
        match res.kind {
            bestdori::ResourceType::Custom => {
                Self::resolve_custom(&res.path, webgal::ResourceType::Background)
            }
            bestdori::ResourceType::Bandori => {
                Self::resolve_bundle(&res.path, webgal::ResourceType::Background)
            }
            _ => None,
        }
    }

    fn resolve_bgm(res: &bestdori::Resource) -> Option<webgal::Resource> {
        match res {
            bestdori::Resource {
                kind: bestdori::ResourceType::Custom,
                path,
            } => Self::resolve_custom(path, webgal::ResourceType::Bgm),

            // 从数据包获取 bgm
            bestdori::Resource {
                kind: bestdori::ResourceType::Bandori,
                path: bestdori::ResourcePath::File { file, bundle: None },
            } => {
                let file = format!("{file}{RESOURCE_SOUND_EXTEND}");
                Some(webgal::Resource {
                    kind: webgal::ResourceType::Bgm,
                    url: format!(
                        "{BESTDORI_ASSET_URL_ROOT}{}_rip/{file}",
                        lower_first_alphabetic(&file)
                    ),
                    path: file,
                })
            }

            _ => None,
        }
    }

    fn resolve_se(res: &bestdori::Resource) -> Option<webgal::Resource> {
        match res {
            bestdori::Resource {
                kind: bestdori::ResourceType::Custom,
                path,
            } => Self::resolve_custom(path, webgal::ResourceType::Vocal),

            // 从数据包获取 se
            bestdori::Resource {
                kind: bestdori::ResourceType::Bandori,
                path:
                    bestdori::ResourcePath::File {
                        file,
                        bundle: Some(bundle),
                    },
            } => {
                let file = format!("{file}{RESOURCE_SOUND_EXTEND}");
                Some(webgal::Resource {
                    kind: webgal::ResourceType::Vocal,
                    url: format!("{BESTDORI_ASSET_URL_ROOT}{bundle}_rip/{file}"),
                    path: file,
                })
            }

            // 从公用资源获取 se
            bestdori::Resource {
                kind: bestdori::ResourceType::Common,
                path: bestdori::ResourcePath::File { file, bundle: None },
            } => {
                let file = format!("{file}{RESOURCE_SOUND_EXTEND}");
                Some(webgal::Resource {
                    kind: webgal::ResourceType::Vocal,
                    url: format!("{BESTDORI_ASSET_URL_SE}{file}"),
                    path: file,
                })
            }

            _ => None,
        }
    }

    // ---------------- resolve ----------------

    /// 解析上传的资源
    fn resolve_custom(
        res: &bestdori::ResourcePath,
        kind: webgal::ResourceType,
    ) -> Option<webgal::Resource> {
        match res {
            bestdori::ResourcePath::Url { url } => Some(webgal::Resource {
                kind,
                url: url.clone(),
                path: gen_name_from_url(url, get_extend! {kind}),
            }),
            _ => None,
        }
    }

    /// 解析带完整路径的资源
    fn resolve_bundle(
        res: &bestdori::ResourcePath,
        kind: webgal::ResourceType,
    ) -> Option<webgal::Resource> {
        match res {
            bestdori::ResourcePath::File {
                file,
                bundle: Some(bundle),
            } => Some(webgal::Resource {
                kind,
                url: format!("{BESTDORI_ASSET_URL_ROOT}{bundle}_rip/{file}"),
                path: format!("{bundle}-{file}{}", get_extend! {kind}),
            }),
            _ => None,
        }
    }
}

impl Resolve for Resolver {
    fn resolve_normal(
        &mut self,
        res: &bestdori::Resource,
        kind: ResourceType,
    ) -> ResolveResult<ResourceEntry> {
        self.get_or_insert(ResourceKey::Normal(res.clone(), kind), || {
            Self::resolve(res, kind).ok_or_else(|| ResolveError {
                kind,
                resource: res.clone(),
            })
        })
    }

    fn resolve_model(&mut self, costume: &str) -> ResourceEntry {
        self.get_or_insert(ResourceKey::Model(costume.to_string()), || {
            Ok(webgal::Resource {
                kind: webgal::ResourceType::Figure,
                url: format!(
                    "{BESTDORI_ASSET_URL_MODEL}{costume}_rip/{BESTDORI_ASSET_URL_MODEL_BUILDER}"
                ),
                path: format!("{costume}/"),
            })
        })
        .unwrap() // :(
    }
}
