//! 模板字符串格式化

use std::borrow::Cow;

// use once_cell::sync::Lazy;
// use regex::Regex;

use crate::{error::*, token::*};

// /// 捕获 ${var:regex} 的正则
// static CAPTURE_REPLACE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{(.*?)\}").unwrap());

/// 模板字符串格式化器
///
/// 模板串为包含了 ${var:regex} / ${var} 的字符串.
///
/// 格式化时, 将对应 var 通过传入的闭包替换为对应字符串 (失败时采用模式串).
/// 若有正则, 则捕获并使用第一个提取项 (失败时采用变量内容).
pub struct TemplateParser(Vec<Token>);

impl TemplateParser {
    /// 从模板字符串创建格式化器
    ///
    /// 替换模式生成失败时转变为文本模式, 并返回报错
    pub fn new(template: &str) -> (Self, Vec<Error>) {
        let mut tokens = Vec::new();
        let mut errs = Vec::new();
        let chars: Vec<char> = template.chars().collect();
        let n = chars.len();
        let mut i = 0;

        while i < n {
            // 查找下一个 ${
            let start = i;
            while i < n && !(chars[i] == '$' && i + 1 < n && chars[i + 1] == '{') {
                i += 1;
            }

            // 处理文本部分
            if start < i {
                tokens.push(Token::new_text(&template[start..i]));
            }

            // 检查是否找到了 ${
            if i >= n || !(chars[i] == '$' && i + 1 < n && chars[i + 1] == '{') {
                break;
            }

            // 查找对应的 }
            let mut brace_count = 1;
            let mut j = i + 2; // 跳过 ${

            while j < n && brace_count > 0 {
                match chars[j] {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    _ => {}
                }
                j += 1;
            }

            if brace_count == 0 {
                // 成功找到了完整的 ${...}
                let replace_end = j - 1; // j 指向 } 后的字符
                let replace_content = &template[i..=replace_end];

                let (token, err) = Token::new_replace(replace_content);
                tokens.push(token);
                if let Some(e) = err {
                    errs.push(e);
                }

                i = j; // 移动到 } 之后
            } else {
                // 未找到匹配的 }, 将 $ 作为普通文本处理
                tokens.push(Token::new_text(&template[i..i + 1]));
                i += 1;
            }
        }

        (Self(tokens), errs)
    }

    /// 格式化模板串
    ///
    /// 替换模式失败时返回对应的模板串, 并返回报错
    pub fn parse<F>(&self, mut map: F) -> (String, Vec<Error>)
    where
        F: FnMut(&str) -> Option<Cow<'_, str>>,
    {
        let (strs, errs): (Vec<_>, Vec<_>) =
            self.0.iter().map(|token| token.parse(&mut map)).unzip();
        (
            strs.into_iter().collect(),
            errs.into_iter().flatten().collect(),
        )
    }
}
