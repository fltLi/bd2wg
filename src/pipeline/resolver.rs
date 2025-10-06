//! bestdoli 资源解析

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::fs::File;
use std::mem;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};

use serde::Deserialize;

use super::definition::*;
use crate::constant::*;
use crate::error::*;
use crate::models::{
    bestdoli::{Address, AddressPath, AddressType},
    live2d::{self, Bundle, Model, ModelBundle},
};

/// 常规资源解析结果
pub enum ResolveCommonResult {
    New(Rc<Resource>),
    Existing(*const Resource),
}

impl AsRef<Resource> for ResolveCommonResult {
    fn as_ref(&self) -> &Resource {
        match self {
            ResolveCommonResult::New(resource) => resource.as_ref(),
            ResolveCommonResult::Existing(ptr) => unsafe { &**ptr },
        }
    }
}

/// Live2D 资源解析结构
pub enum ResolveModelResult {
    Normal(Rc<Resource>),
    Bind {
        url: String,
        task: Box<dyn Fn(Vec<u8>) -> Vec<Resource> + Send + 'static>,
    },
    Existing,
}

/// bestdoli 资源解析器
///
/// - 在 Purify 过程中, Resolver 收集资源并规范资源路径
/// - 在 Extractor 过程中, Resolver 将引导 Live2D 支持
pub trait Resolver {
    fn resolve_bgm(&mut self, addr: &Address) -> Result<ResolveCommonResult>;

    fn resolve_se(&mut self, addr: &Address) -> Result<ResolveCommonResult>;

    fn resolve_background(&mut self, addr: &Address) -> Result<ResolveCommonResult>;

    /// 角色卡牌
    fn resolve_cardstill(&mut self, addr: &Address) -> Result<ResolveCommonResult>;

    /// 模型 (衣装) 资源
    fn resolve_model(&mut self, character: u8, model: &mut String) -> Result<ResolveModelResult>;

    /// 角色通用动作资源
    fn resolve_motion(&mut self, character: u8, motion: &str) -> Result<ResolveModelResult>;

    /// 角色通用表情资源
    fn resolve_expression(&mut self, character: u8, expression: &str)
    -> Result<ResolveModelResult>;

    /// 生成 webgal live2d 配置文件
    ///
    /// 请确保模型下载任务已完成, 否则可能会漏掉模型.
    fn get_model_config(&self) -> Vec<ModelConfig>;

    /// 返回已记录的解析错误 (捆绑任务)
    fn take_error(&mut self) -> Vec<ResolveError>;
}

fn create_resource(root: Root, url: String, extend: &str) -> Rc<Resource> {
    let path = url_to_filepath(&url, extend);
    Rc::new(Resource {
        root,
        url: Some(url),
        path,
    })
}

/// 通过 url 生成路径
fn url_to_filepath(url: &str, extend: &str) -> String {
    url.chars()
        .map(|c| match c {
            ':' | '?' | '*' | '"' | '<' | '>' | '|' | '\\' | '/' | ' ' => '_',
            c => c,
        })
        .chain(extend.chars())
        .collect()
}

/// 默认解析器配置
#[derive(Deserialize)]
pub struct BestdoliConfig {
    pub bundle_root: String,
    pub bgm_bundle: String,
    pub se_common: String,
    pub live2d_bundle: String,
}

#[derive(Default)]
struct CommonRecord {
    bgm: HashMap<Address, Rc<Resource>>,
    se: HashMap<Address, Rc<Resource>>,
    background: HashMap<Address, Rc<Resource>>,
    cardstill: HashMap<Address, Rc<Resource>>,
}

struct Character {
    model: Arc<RwLock<HashMap<String, Model>>>,
    motion: HashSet<String>,
    expression: HashSet<String>,
}

impl Character {
    fn new() -> Self {
        Self {
            model: Arc::new(RwLock::new(HashMap::new())),
            motion: HashSet::new(),
            expression: HashSet::new(),
        }
    }
}

struct ModelRecord {
    pending: Arc<RwLock<HashSet<(u8, String)>>>,
    model: HashMap<u8, Character>,
    character: HashMap<u8, String>, // 编号 -> 角色
}

impl Default for ModelRecord {
    fn default() -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashSet::new())),
            model: HashMap::new(),
            character: HashMap::new(),
        }
    }
}

/// 默认 bestdoli 资源解析器
///
/// 设计原因, 目前 motion 和 expression 不能是特殊服装.  
/// 那么如何解决这个问题呢? 记录一个 character -> model 的上下文并启用 download_lazy.
///
/// $\uarr$ 最后还是维护了啊...
pub struct DefaultResolver {
    root: String,
    config: BestdoliConfig,
    common: CommonRecord, // 常规记录
    model: ModelRecord,   // 模型记录
    error: Arc<Mutex<Vec<ResolveError>>>,
}

impl DefaultResolver {
    /// 读取默认配置并启动
    pub fn new(root: String) -> Result<Self> {
        Ok(Self::with_config(
            root,
            serde_json::from_reader(File::open_buffered(RESOLVE_CONFIG)?)?,
        ))
    }

    pub fn with_config(root: String, config: BestdoliConfig) -> Self {
        Self {
            root,
            config,
            common: CommonRecord::default(),
            model: ModelRecord::default(),
            error: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_character_root(character: u8) -> String {
        format!("{character:03}/")
    }

    fn get_model_root(character: u8, model: &str) -> String {
        format!("{}model/{model}/", Self::get_character_root(character))
    }

    fn get_motion_path(character: u8, motion: &str) -> String {
        format!("{}motion/{motion}.mtn", Self::get_character_root(character))
    }

    fn get_expression_path(character: u8, expression: &str) -> String {
        format!(
            "{}expression/{expression}.exp.json",
            Self::get_character_root(character)
        )
    }

    fn get_live2d_general_url(&self, character: u8, file: &str) -> String {
        Self::bundle_to_url_with_root(
            &self.config.bundle_root,
            &format!("{}{character:03}_general", self.config.live2d_bundle),
            file,
        )
    }

    fn bundle_to_url_with_root(root: &str, bundle: &str, file: &str) -> String {
        format!("{root}{bundle}_rip/{file}")
    }

    /// 合成数据包链接
    fn bundle_to_url(&self, bundle: &str, file: &str, extend: &str) -> String {
        Self::bundle_to_url_with_root(&self.config.bundle_root, bundle, &format!("{file}{extend}"))
    }

    /// 尝试提取非数据包链接
    fn try_resolve_custom(addr: &Address) -> Option<String> {
        if addr.kind == AddressType::Custom
            && let AddressPath::Url { url } = &addr.address
        {
            Some(url.clone())
        } else {
            None
        }
    }

    /// 尝试提取数据包链接
    fn try_resolve_bundle(&self, addr: &Address, extend: &str) -> Option<String> {
        if addr.kind == AddressType::Bandori
            && let AddressPath::File {
                file,
                bundle: Some(bundle),
            } = &addr.address
        {
            Some(self.bundle_to_url(bundle, file, extend))
        } else {
            None
        }
    }
}

impl Resolver for DefaultResolver {
    fn resolve_bgm(&mut self, addr: &Address) -> Result<ResolveCommonResult> {
        // 使用 Entry 处理生命周期很麻烦, 暂时搁置.
        if let Some(existing) = self.common.bgm.get(addr) {
            return Ok(ResolveCommonResult::Existing(Rc::as_ptr(existing)));
        }

        let res = Self::try_resolve_custom(addr)
            .or_else(|| {
                self.try_resolve_bundle(addr, ".mp3").map(|mut s| {
                    // 数据包名称可能涉及大小写转换.
                    if let Some(last_slash) = s.rfind('/')
                        && let Some(second_last_slash) = s[..last_slash].rfind('/')
                        && let Some((pos, c)) = s[second_last_slash..]
                            .char_indices()
                            .skip(1)
                            .find(|(_, c)| c.is_ascii_alphabetic())
                    {
                        let replace_pos = second_last_slash + pos;
                        if c.is_ascii_uppercase() {
                            s.replace_range(
                                replace_pos..replace_pos + c.len_utf8(),
                                &c.to_ascii_lowercase().to_string(),
                            );
                        }
                    }
                    s
                })
            })
            .map(|url| create_resource(Root::Bgm, url, ".mp3"))
            .ok_or_else(|| ResolveError::Common {
                kind: ResolveCommonKind::Bgm,
                addr: addr.clone(),
            })?;

        self.common.bgm.insert(addr.clone(), res.clone());
        Ok(ResolveCommonResult::New(res))
    }

    fn resolve_se(&mut self, addr: &Address) -> Result<ResolveCommonResult> {
        if let Some(existing) = self.common.se.get(addr) {
            return Ok(ResolveCommonResult::Existing(Rc::as_ptr(existing)));
        }

        let res = Self::try_resolve_custom(addr)
            .or_else(|| self.try_resolve_bundle(addr, ".mp3"))
            .or_else(|| {
                if let Address {
                    kind: AddressType::Common,
                    address: AddressPath::File { file, bundle: None },
                } = addr
                {
                    Some(format!("{}{file}.mp3", self.config.se_common))
                } else {
                    None
                }
            })
            .map(|url| create_resource(Root::Vocal, url, ".mp3"))
            .ok_or_else(|| ResolveError::Common {
                kind: ResolveCommonKind::Se,
                addr: addr.clone(),
            })?;

        self.common.se.insert(addr.clone(), res.clone());
        Ok(ResolveCommonResult::New(res))
    }

    fn resolve_background(&mut self, addr: &Address) -> Result<ResolveCommonResult> {
        if let Some(existing) = self.common.background.get(addr) {
            return Ok(ResolveCommonResult::Existing(Rc::as_ptr(existing)));
        }

        let res = Self::try_resolve_custom(addr)
            .or_else(|| self.try_resolve_bundle(addr, ".png"))
            .map(|url| create_resource(Root::Background, url, ".png"))
            .ok_or_else(|| ResolveError::Common {
                kind: ResolveCommonKind::Background,
                addr: addr.clone(),
            })?;

        self.common.background.insert(addr.clone(), res.clone());
        Ok(ResolveCommonResult::New(res))
    }

    fn resolve_cardstill(&mut self, addr: &Address) -> Result<ResolveCommonResult> {
        if let Some(existing) = self.common.background.get(addr) {
            return Ok(ResolveCommonResult::Existing(Rc::as_ptr(existing)));
        }

        let res = Self::try_resolve_custom(addr)
            .or_else(|| self.try_resolve_bundle(addr, ".png"))
            .map(|url| create_resource(Root::Background, url, ".png"))
            .ok_or_else(|| ResolveError::Common {
                kind: ResolveCommonKind::Background,
                addr: addr.clone(),
            })?;

        self.common.background.insert(addr.clone(), res.clone());
        Ok(ResolveCommonResult::New(res))
    }

    fn resolve_model(&mut self, character: u8, model: &mut String) -> Result<ResolveModelResult> {
        if model.is_empty() {
            if let Some(v) = self.model.character.get(&character) {
                *model = v.clone();
            }
        } else {
            self.model.character.insert(character, model.clone());
        }

        let (exist, dict) = match self.model.model.entry(character) {
            Entry::Occupied(o) => (
                o.get().model.read().unwrap().contains_key(model)
                    || self
                        .model
                        .pending
                        .read()
                        .unwrap()
                        .contains(&(character, model.clone())),
                o.get().model.clone(),
            ),
            Entry::Vacant(v) => (false, v.insert(Character::new()).model.clone()),
        };

        let root_ = Self::get_model_root(character, model);

        let res = if !exist {
            let dict = dict.clone();
            let mkey = (character, model.clone());
            let pend = self.model.pending.clone();
            let errs = self.error.clone();
            let root = root_.clone();
            let head = self.config.bundle_root.clone();
            let bundle_to_url: impl Fn(&Bundle) -> String =
                move |bundle| Self::bundle_to_url_with_root(&head, &bundle.bundle, &bundle.file);
            let bundle_to_path: impl Fn(&Bundle) -> String =
                |bundle| format!("live2d/{}", bundle.file);
            let bundle_to_full_path: impl Fn(&Bundle) -> String =
                move |bundle| format!("{root}/{}", bundle_to_path(bundle));

            pend.write().unwrap().insert(mkey.clone());

            ResolveModelResult::Bind {
                url: self.bundle_to_url(
                    &format!("{}{model}", &self.config.live2d_bundle),
                    "buildData",
                    ".asset",
                ),
                task: Box::new(move |bytes| {
                    pend.write().unwrap().remove(&mkey);

                    match ModelBundle::from_bytes(&bytes) {
                        Ok(bundle) => {
                            let mut items = Vec::with_capacity(4);
                            let ModelBundle {
                                model,
                                physics,
                                textures,
                            } = bundle;

                            let minfo = Model {
                                model: bundle_to_path(&model),
                                physics: bundle_to_path(&physics),
                                textures: textures
                                    .into_iter()
                                    .map(|texture| {
                                        items.push(Resource {
                                            root: Root::Figure,
                                            url: Some(bundle_to_url(&texture)),
                                            path: bundle_to_full_path(&texture),
                                        });
                                        bundle_to_path(&texture)
                                    })
                                    .collect(),
                            };
                            dict.write().unwrap().insert(mkey.1.clone(), minfo);

                            items.push(Resource {
                                root: Root::Figure,
                                url: Some(bundle_to_url(&model)),
                                path: bundle_to_full_path(&model),
                            });

                            items.push(Resource {
                                root: Root::Figure,
                                url: Some(bundle_to_url(&physics)),
                                path: bundle_to_full_path(&physics),
                            });

                            items
                        }
                        Err(err) => {
                            let mut errs = errs.lock().unwrap();
                            errs.push(ResolveError::Live2D {
                                kind: ResolveLive2DKind::Motion,
                                character,
                                attr: err.to_string(),
                            });
                            vec![]
                        }
                    }
                }),
            }
        } else {
            ResolveModelResult::Existing
        };

        *model = format!("{root_}model.json");
        Ok(res)
    }

    fn resolve_motion(&mut self, character: u8, motion: &str) -> Result<ResolveModelResult> {
        let exist = match self.model.model.entry(character) {
            Entry::Occupied(mut o) => !o.get_mut().motion.insert(motion.to_string()),
            Entry::Vacant(v) => {
                v.insert(Character::new()).motion.insert(motion.to_string());
                false
            }
        };

        if exist {
            Ok(ResolveModelResult::Existing)
        } else {
            Ok(ResolveModelResult::Normal(Rc::new(Resource {
                root: Root::Figure,
                url: Some(self.get_live2d_general_url(character, &format!("{motion}.mtn"))),
                path: Self::get_motion_path(character, motion),
            })))
        }
    }

    fn resolve_expression(
        &mut self,
        character: u8,
        expression: &str,
    ) -> Result<ResolveModelResult> {
        let exist = match self.model.model.entry(character) {
            Entry::Occupied(mut o) => !o.get_mut().expression.insert(expression.to_string()),
            Entry::Vacant(v) => {
                v.insert(Character::new())
                    .motion
                    .insert(expression.to_string());
                false
            }
        };

        if exist {
            Ok(ResolveModelResult::Existing)
        } else {
            Ok(ResolveModelResult::Normal(Rc::new(Resource {
                root: Root::Figure,
                url: Some(
                    self.get_live2d_general_url(character, &format!("{expression}.exp.json")),
                ),
                path: Self::get_expression_path(character, expression),
            })))
        }
    }

    fn get_model_config(&self) -> Vec<ModelConfig> {
        self.model
            .model
            .iter()
            .flat_map(|(id, chara)| {
                let motion = Rc::new(
                    chara
                        .motion
                        .iter()
                        .map(|motion| {
                            (
                                motion.clone(),
                                live2d::Motion {
                                    file: format!("../motion/{motion}.mtn"),
                                }
                                .into(),
                            )
                        })
                        .collect::<Vec<(String, Vec<live2d::Motion>)>>(),
                );
                let expression = Rc::new(
                    chara
                        .expression
                        .iter()
                        .map(|expression| live2d::Expression {
                            name: expression.clone(),
                            file: format!("../expression/{expression}.exp.json"),
                        })
                        .collect::<Vec<live2d::Expression>>(),
                );

                chara
                    .model
                    .read()
                    .unwrap()
                    .iter()
                    .map(|(name, model)| ModelConfig {
                        root: Root::Figure,
                        path: format!("{}/model.json", Self::get_model_root(*id, name)),
                        data: live2d::ModelConfig::new(
                            model.clone(),
                            motion.clone(),
                            expression.clone(),
                        ),
                    })
                    .collect::<Vec<ModelConfig>>()
            })
            .collect::<Vec<ModelConfig>>()
    }

    fn take_error(&mut self) -> Vec<ResolveError> {
        let mut errors = self.error.lock().unwrap();
        mem::take(&mut errors)
    }
}
