//! webgal 项目组装

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::path::PathBuf;

use super::definition::*;
use crate::constant::*;
use crate::error::*;
use crate::models::webgal::Action;

/// webgal 项目组装器
///
/// - 创建并写入 webgal 场景
/// - 写入 live2d 配置文件
pub trait Extractor {
    /// 切入场景文件
    fn change_scene(&mut self, scene: &str) -> Result<()>;

    /// 写入一条指令
    fn write_action(&mut self, action: &Action) -> Result<()>;

    /// 写入 webgal live2d 配置文件
    fn write_model_config(&mut self, config: &ModelConfig) -> Result<()>;
}

/// 创建文件并返回写入器
fn create_file_writer(path: &Path) -> Result<BufWriter<File>> {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(File::create_buffered(path)?)
}

/// 默认 webgal 项目组装器
pub struct DefaultExtractor {
    root: String,
    scene: Option<BufWriter<File>>, // 不为空
}

impl DefaultExtractor {
    /// 创建构建器并获取初始场景文件句柄
    pub fn new(root: String, scene: &str) -> Result<Self> {
        let mut extractor = Self { root, scene: None };
        extractor.change_scene(scene)?;
        Ok(extractor)
    }
}

impl Extractor for DefaultExtractor {
    fn change_scene(&mut self, scene: &str) -> Result<()> {
        let path = PathBuf::from(&self.root)
            .join(Root::Scene.to_string())
            .join(scene);
        self.scene.replace(create_file_writer(&path)?);
        Ok(())
    }

    fn write_action(&mut self, action: &Action) -> Result<()> {
        writeln!(self.scene.as_mut().unwrap(), "{action}")?;
        Ok(())
    }

    fn write_model_config(&mut self, config: &ModelConfig) -> Result<()> {
        let path = PathBuf::from(&self.root).join(config.get_full_path());
        fs::write(path, serde_json::to_string(&config.data)?)?;
        Ok(())
    }
}
