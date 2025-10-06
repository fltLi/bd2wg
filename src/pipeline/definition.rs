//! bd2wg 工作管线公共定义

use std::rc::Rc;

use strum::Display;

use crate::models::live2d;

#[derive(Debug, Display)]
#[strum(serialize_all = "camelCase")]
pub enum Root {
    Background,
    Bgm,
    Vocal,
    Figure,
    Scene,
}

macro_rules! impl_get_full_path {
    ($name:ident) => {
        paste::paste! {
            impl $name {
                /// 获取完整 (相对) 路径
                pub fn get_full_path(&self) -> String {
                    format!("{}/{}", self.root, self.path)
                }
            }
        }
    };
}

/// 资源描述
#[derive(Debug)]
pub struct Resource {
    pub root: Root,
    pub url: Option<String>,
    pub path: String,
}

impl_get_full_path! {Resource}

/// live2d 配置描述
pub struct ModelConfig {
    pub root: Root,
    pub path: String,
    pub data: live2d::ModelConfig,
}

impl_get_full_path! {ModelConfig}

pub trait BindTask: Fn(Vec<u8>) -> Vec<Resource> + Send + 'static {}

pub trait LazyTask: Fn() -> Resource + Send + 'static {}

// Blanket impls so Box<dyn ...> satisfy the traits
impl<T> BindTask for T where T: Fn(Vec<u8>) -> Vec<Resource> + Send + 'static {}
impl<T> LazyTask for T where T: Fn() -> Resource + Send + 'static {}

/// 资源任务
pub enum ResourceTask {
    Task(Rc<Resource>),
    Bind {
        url: String,
        task: Box<dyn Fn(Vec<u8>) -> Vec<Resource> + Send + 'static>,
    },
}
