//! 重定向

// TODO: 设置规则时在子线程编译正则

use crate::{error::RedirectError, models::redirect::RedirectRules};

pub type RedirectResult<T> = Result<T, RedirectError>;

/// 模型动作 / 表情重定向
pub trait MotionRedirect {
    fn redirect_motion(&self, motion: &str) -> RedirectResult<String>;

    fn redirect_expression(&self, expression: &str) -> RedirectResult<String>;
}

/// 资源重定向
pub trait Redirect {
    type MotionRedirector: MotionRedirect;

    /// 添加重定向规则
    fn add_rules(&mut self, rules: &RedirectRules);

    /// 模型重定向
    fn redirect_model(&self, costume: &str) -> Option<Self::MotionRedirector>;
}
