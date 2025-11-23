//! WebGAL 资源

use std::path::{Path, PathBuf};

/// WebGAL 资源
pub trait Asset {
    /// WebGAL 脚本资源路径
    fn relative_path(&self) -> String;

    /// 资源绝对路径
    fn absolute_path<P: AsRef<Path>>(&self, root: P) -> PathBuf {
        root.as_ref().join(self.relative_path())
    }
}
