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

/// 为支持 Serialize 的对象实现 Display
#[macro_export]
macro_rules! impl_display_for_serde {
    ($name:ident) => {
        paste::paste! {
            impl Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", serde_json::to_string(self).map_err(|_| fmt::Error)?)
                }
            }
        }
    };
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
