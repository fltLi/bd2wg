//! 辅助工具

use std::fs;
use std::path::Path;

use bytes::Bytes;
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;

/// 为 Handle 实现 cancel Drop
#[macro_export]
macro_rules! impl_drop_for_handle {
    ($t:ty) => {
        impl Drop for $t {
            fn drop(&mut self) {
                if !self.is_finished() {
                    self.cancel();
                }
            }
        }
    };
}

/// 为 Serialize 实现 Display
#[macro_export]
macro_rules! impl_display_for_serde {
    ($t:ty) => {
        paste::paste! {
            impl Display for $t {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", serde_json::to_string(self).map_err(|_| fmt::Error)?)
                }
            }
        }
    };
}

/// 为 AsRef 实现 Deref
#[macro_export]
macro_rules! impl_deref_for_asref {
    ($t:ty, $to:ty) => {
        paste::paste! {
            impl Deref for $t {
                type Target = $to;

                fn deref(&self) -> &Self::Target {
                    self.as_ref()
                }
            }
        }
    };
}

/// 执行表达式, 返回 Ok(())
#[macro_export]
macro_rules! return_ok {
    ($expr:expr) => {{
        let _ = $expr;
        Ok(())
    }};
}

/// 从请求头快速创建 Client
pub fn new_client_with_headers(headers: HeaderMap) -> reqwest::Result<Client> {
    Client::builder().default_headers(headers).build()
}

/// 创建完整路径, 将字节写入文件
pub fn create_and_write<B: AsRef<[u8]>>(bytes: &B, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

/// 从 url 生成唯一路径
pub fn gen_name_from_url(url: &str, extend: &str) -> String {
    url.chars()
        .map(|c| match c {
            ':' | '?' | '*' | '"' | '<' | '>' | '|' | '\\' | '/' | ' ' => '_',
            c => c,
        })
        .chain(extend.chars())
        .collect()
}

/// 将第一个英文字母变为小写
pub fn lower_first_alphabetic(s: &str) -> String {
    let mut find = false;
    s.chars()
        .map(|mut c| {
            if !find && c.is_ascii_alphabetic() {
                find = true;
                c.make_ascii_lowercase();
            }
            c
        })
        .collect()
}
