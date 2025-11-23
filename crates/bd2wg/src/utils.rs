//! 辅助工具

/// 为支持 Serialize 的对象实现 Display
#[macro_export]
macro_rules! impl_serde_display {
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
