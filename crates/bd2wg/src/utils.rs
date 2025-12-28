//! 辅助工具

use std::{fs, path::Path};

use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::Value;

// /// 默认请求头路径
// pub const DEFAULT_HEADER_PATH: &str = "./assets/header.json";

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

/// 为元组型结构体实现 iter 和 into_iter
#[macro_export]
macro_rules! impl_iter_for_tuple {
    ($t:ty, $inner:ty) => {
        paste::paste! {
            impl $t {
                /// 枚举内部元素
                pub fn iter(&self) -> impl Iterator<Item = &$inner> {
                    self.0.iter()
                }
            }

            impl IntoIterator for $t {
                type Item = $inner;
                type IntoIter = std::vec::IntoIter<$inner>;

                fn into_iter(self) -> Self::IntoIter {
                    self.0.into_iter()
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

/// 当原子量为 true 时 panic
#[macro_export]
macro_rules! false_or_panic {
    ($atom:expr) => {
        false_or_panic! {$atom, "canceled."}
    };
    ($atom:expr, $text:expr) => {
        if $atom.load(std::sync::atomic::Ordering::Relaxed) {
            panic!($text)
        }
    };

    ($atom:ident) => {
        false_or_panic! {$atom, "canceled."}
    };
    ($atom:ident, $text:literal) => {
        if $atom.load(std::sync::atomic::Ordering::Relaxed) {
            panic!($text)
        }
    };
}

/// 从请求头快速创建 Client
pub fn new_client_with_header(header: HeaderMap) -> reqwest::Result<Client> {
    #[cfg(feature = "wider_compression")]
    {
        Client::builder().default_headers(header).build()
    }

    #[cfg(not(feature = "wider_compression"))]
    {
        let mut defaults = header;
        defaults.remove(reqwest::header::ACCEPT_ENCODING);
        Client::builder().default_headers(defaults).build()
    }
}

/// 创建完整路径, 将字节写入文件
pub fn create_and_write(bytes: impl AsRef<[u8]>, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

/// 尝试移除后缀
///
/// 改为泛型是 unstable, 因此固定 suffix 为 &str
pub fn maybe_strip_suffix<'a>(s: &'a str, suffix: &str) -> &'a str {
    s.strip_suffix(suffix).unwrap_or(s)
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

/// 根据 `Content-Encoding` 尝试解压字节流 (作为回退解码)
#[cfg(feature = "wider_compression")]
pub fn maybe_decompress_bytes(bytes: &[u8], encoding: &str) -> std::io::Result<Vec<u8>> {
    use std::io::{Cursor, Read};

    use brotli2::read::BrotliDecoder;
    use zstd::stream::read::Decoder as ZstdDecoder;

    let enc = encoding.to_lowercase();

    if enc.contains("zstd") {
        let cursor = Cursor::new(bytes);
        let mut dec = ZstdDecoder::new(cursor)?;
        let mut out = Vec::new();
        dec.read_to_end(&mut out)?;

        Ok(out)
    } else if enc.contains("br") || enc.contains("brotli") {
        let cursor = Cursor::new(bytes);
        let mut dec = BrotliDecoder::new(cursor);
        let mut out = Vec::new();
        dec.read_to_end(&mut out)?;

        Ok(out)
    } else {
        Ok(bytes.to_vec())
    }
}

/// 从 json 构建 HeaderMap
pub fn new_header_from_json(val: &Value) -> anyhow::Result<HeaderMap> {
    let mut map = HeaderMap::new();

    if let Value::Object(obj) = val {
        for (k, val) in obj {
            if k.starts_with(':') {
                continue;
            }

            let s = if let Some(s) = val.as_str() {
                s.to_string()
            } else {
                val.to_string()
            };

            let name = HeaderName::from_bytes(k.as_bytes())?;
            let hv = HeaderValue::from_str(&s)?;
            map.insert(name, hv);
        }
    }

    Ok(map)
}

/// 解析 json 并构建 HeaderMap
pub fn new_header_from_bytes(bytes: &[u8]) -> anyhow::Result<HeaderMap> {
    new_header_from_json(&serde_json::from_slice(bytes)?)
}

/// 默认请求头文件
#[cfg(feature = "default_header")]
const HEADER_JSON: &[u8] = include_bytes!("../assets/header.json");

/// 解析默认请求头
#[cfg(feature = "default_header")]
pub fn default_header() -> anyhow::Result<HeaderMap> {
    new_header_from_bytes(HEADER_JSON)
}
