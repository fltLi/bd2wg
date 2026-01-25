//! 模板字符串片段

use std::borrow::Cow;

use regex::Regex;

use crate::error::*;

pub enum Token {
    Text(String),
    Replace(#[allow(private_interfaces)] ReplaceToken),
}

impl Token {
    /// 创建文本模式
    pub fn new_text(text: &str) -> Self {
        Self::Text(text.to_string())
    }

    /// 创建替换模式, 失败时返回文本模式
    pub fn new_replace(template: &str) -> (Self, Option<Error>) {
        match ReplaceToken::new(template) {
            Ok(r) => (Self::Replace(r), None),
            Err(e) => (Self::new_text(template), Some(e)),
        }
    }

    pub fn parse<F>(&self, map: &mut F) -> (Cow<'_, str>, Option<Error>)
    where
        F: FnMut(&str) -> Option<Cow<'_, str>>,
    {
        match self {
            Self::Text(t) => (Cow::Borrowed(t), None),
            Self::Replace(r) => r.parse(map),
        }
    }
}

struct ReplaceToken {
    template: String,
    variable_len: usize, // 变量的终点位置
    regex: Option<Regex>,
}

impl ReplaceToken {
    /// 创建替换模式
    ///
    /// 模板串为 "${var:regex}" (含 ${} 边界)
    fn new(template: &str) -> Result<Self, Error> {
        // 分割变量和正则
        let len = template.len();
        let (variable_len, regex) = match template.find(':') {
            None => (len - 1, None),
            // 尝试编译正则
            Some(p) => (p, Some(Regex::new(&template[p + 1..len - 1])?)),
        };

        Ok(Self {
            template: template.to_string(),
            variable_len,
            regex,
        })
    }

    fn variable(&self) -> &str {
        &self.template[2..self.variable_len]
    }

    fn template_cow(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.template)
    }

    /// 格式化模板, 失败时返回模板
    fn parse<F>(&self, map: &mut F) -> (Cow<'_, str>, Option<Error>)
    where
        F: FnMut(&str) -> Option<Cow<'_, str>>,
    {
        // 尝试获取变量
        let var = match map(self.variable()) {
            Some(v) => v,
            None => {
                return (
                    self.template_cow(),
                    Some(Error::VariableNotFound(self.template.clone())),
                );
            }
        };

        // 尝试捕获正则
        let res = match &self.regex {
            Some(r) => r.captures(&var).and_then(|var| var.get(1)),
            None => return (var, None),
        };

        match res {
            Some(r) => (Cow::Owned(r.as_str().to_string()), None),
            None => (
                self.template_cow(),
                Some(Error::VariableParse {
                    template: self.template.clone(),
                    variable: var.into_owned(),
                }),
            ),
        }
    }
}
