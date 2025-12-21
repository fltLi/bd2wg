//! WebGAL 脚本序列化
//! 
//! 使用 #[derive(webgal_derive::Actionable)] 为结构体添加序列化功能.

use std::fmt::Display;

// 重新导出派生宏
pub use webgal_derive_macro::Actionable;

/// WebGAL 命令标记特型
pub trait Actionable: Display {}

/// 自定义序列化行为
pub trait ActionCustom {
    fn get_head(&self) -> String {
        String::default()
    }

    fn get_main(&self) -> String {
        String::default()
    }

    fn get_other_args(&self) -> Option<Vec<(String, Option<String>)>> {
        None
    }
}
