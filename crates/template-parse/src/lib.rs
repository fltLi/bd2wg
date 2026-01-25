//! 模板格式化
//! 
//! 提供 TemplateParser 完成模板字符串的填充与格式化

mod error;
mod parser;
mod token;

pub use error::*;
pub use parser::TemplateParser;
