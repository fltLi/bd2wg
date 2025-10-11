//! bd2wg 工作管线

use std::collections::LinkedList;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use super::*;
use crate::constant::WEBGAL_START_SCENE;
use crate::error::*;
use crate::models::bestdori::{self, Story};

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumString, strum::Display)]
pub enum Procedure {
    NotStarted,
    ParsingScript,
    Transpiling,
    WaitingForDownload,
    Extracting,
    Completed,
}

/// 工作状态
pub struct State {
    pub procedure: Procedure,
    pub error: Vec<Error>,
    pub scene_count: u16,
    pub download_task_count: usize,
    pub purified_action_count: usize,
    pub transpiled_action_count: usize,
}

/// bd2wg 工作管线
///
/// - 管线工作不阻塞子线程
/// - 允许通过相关方法查询处理进度
pub trait Pipeline {
    /// 开启处理
    fn process(&mut self) -> Result<()>; // err: 启动出错

    /// 等待完成
    fn wait(&mut self) -> Result<()>;

    /// 获取状态信息
    ///
    /// - 会清空当前存储的错误
    fn take_state(&mut self) -> Result<State>; // err: 并发访问出错
}

struct InnerState {
    procedure: Mutex<Procedure>,
    error: Mutex<Vec<Error>>,
    pub scene_count: AtomicU16,
    pub download_task_count: AtomicUsize,
    pub purified_action_count: AtomicUsize,
    pub transpiled_action_count: AtomicUsize,
}

impl InnerState {
    fn new() -> Self {
        Self {
            procedure: Mutex::new(Procedure::NotStarted),
            error: Mutex::new(Vec::new()),
            scene_count: AtomicU16::new(0),
            download_task_count: AtomicUsize::new(0),
            purified_action_count: AtomicUsize::new(0),
            transpiled_action_count: AtomicUsize::new(0),
        }
    }
}

/// 默认管线实现
pub struct DefaultPipeline {
    story_path: PathBuf,
    project_path: String,
    handle: Option<thread::JoinHandle<()>>,
    paniced: Arc<AtomicBool>,
    state: Arc<InnerState>,
}

impl DefaultPipeline {
    pub fn new(story_path: &Path, project_path: String) -> Self {
        Self {
            story_path: story_path.into(),
            project_path,
            handle: None,
            paniced: Arc::new(AtomicBool::new(false)),
            state: Arc::new(InnerState::new()),
        }
    }

    pub fn is_paniced(&self) -> bool {
        self.paniced.load(Ordering::Relaxed)
    }
}

macro_rules! safe_unwrap_lock {
    ($self:expr, $var:ident) => {
        paste::paste! {
            match $self.state.$var.lock() {
                Ok(v) => v,
                Err(_) => {
                    $self.paniced.store(true, Ordering::Relaxed);
                    Err(PipelineError::Paniced)?
                }
            }
        }
    };
}

macro_rules! load_atomic {
    ($self:expr, $var:ident) => {
        paste::paste! {
            $self.state.$var.load(Ordering::Relaxed)
        }
    };
}

macro_rules! add_atomic {
    ($var:expr) => {
        $var.fetch_add(1, Ordering::Relaxed)
    };
}

impl Pipeline for DefaultPipeline {
    fn process(&mut self) -> Result<()> {
        if self.is_paniced() {
            Err(PipelineError::Paniced)?
        }
        if *self.state.procedure.lock().unwrap() != Procedure::NotStarted {
            Err(PipelineError::BadStart)?
        }

        let (story_path, project_path) = (self.story_path.clone(), self.project_path.clone());
        let state = self.state.clone();
        let paniced = self.paniced.clone();

        self.handle.replace(thread::spawn(move || {
            // 1. 解析
            *state.procedure.lock().unwrap() = Procedure::ParsingScript;
            let story = Story::from_file(&story_path)
                .map_err(|err| {
                    state.error.lock().unwrap().push(err);
                    paniced.store(true, Ordering::Relaxed);
                    panic!("Failed to parse bestdori script from file.")
                })
                .unwrap();

            // 2. 常驻模块
            let mut resolver = DefaultResolver::new(project_path.clone())
                .map_err(|err| {
                    state.error.lock().unwrap().push(err);
                    paniced.store(true, Ordering::Relaxed);
                    panic!("Failed to start resolver.")
                })
                .unwrap();
            let mut downloader = DefaultDownloader::new(project_path.clone())
                .map_err(|err| {
                    state.error.lock().unwrap().push(err);
                    paniced.store(true, Ordering::Relaxed);
                    panic!("Failed to start downloader.")
                })
                .unwrap();
            let mut extractor = DefaultExtractor::new(project_path.clone(), WEBGAL_START_SCENE)
                .map_err(|err| {
                    state.error.lock().unwrap().push(err);
                    paniced.store(true, Ordering::Relaxed);
                    panic!("Failed to start extractor.")
                })
                .unwrap();

            // 3. 预处理模块
            let purify_iter = DefaultPurifier::new(story.0.into_iter(), &mut resolver)
                .filter_map(|result| match result {
                    Ok(result) => Some(result),
                    Err(err) => {
                        state.error.lock().unwrap().push(err);
                        None
                    }
                })
                .filter_map(|result| match result {
                    PurifyResult::Action(action) => {
                        add_atomic! {state.purified_action_count};
                        Some(action)
                    }
                    PurifyResult::ResourceTask(task) => {
                        add_atomic! {state.download_task_count};
                        match task {
                            ResourceTask::Task(resource) => downloader.download(&resource),
                            ResourceTask::Bind { url, task } => {
                                downloader.download_bind(&url, task)
                            }
                        }
                        .map_err(|err| state.error.lock().unwrap().push(err));
                        None
                    }
                });

            // 4. 转译模块
            *state.procedure.lock().unwrap() = Procedure::Transpiling;
            DefaultTranspiler::new(purify_iter)
                .filter_map(|result| match result {
                    Ok(result) => Some(result),
                    Err(err) => {
                        state.error.lock().unwrap().push(err);
                        None
                    }
                })
                .map(|result| match result {
                    TranspileResult::Scene(scene) => {
                        add_atomic! {state.scene_count};
                        extractor.change_scene(&scene)
                    }
                    TranspileResult::Action(action) => {
                        add_atomic! {state.transpiled_action_count};
                        extractor.write_action(&action)
                    }
                })
                .for_each(|result| {
                    if let Err(err) = result {
                        state.error.lock().unwrap().push(err);
                    }
                });

            // 5. 打包
            *state.procedure.lock().unwrap() = Procedure::WaitingForDownload;
            downloader.wait();
            *state.procedure.lock().unwrap() = Procedure::Extracting;
            resolver
                .get_model_config()
                .iter()
                .map(|config| extractor.write_model_config(config))
                .for_each(|result| {
                    if let Err(err) = result {
                        state.error.lock().unwrap().push(err);
                    }
                });

            // 6. 错误收集
            state.error.lock().unwrap().extend(
                resolver
                    .take_error()
                    .into_iter()
                    .map(|err| err.into())
                    .chain(downloader.take_error().into_iter().map(|err| err.into())),
            );
            *state.procedure.lock().unwrap() = Procedure::Completed;
        }));

        Ok(())
    }

    fn wait(&mut self) -> Result<()> {
        // 等待工作线程完成
        if let Some(handle) = self.handle.take()
            && let Err(_) = handle.join()
        {
            self.paniced.store(true, Ordering::Relaxed);
            Err(PipelineError::Paniced)?
        }
        Ok(())
    }

    fn take_state(&mut self) -> Result<State> {
        if self.is_paniced() {
            Err(PipelineError::Paniced)?
        }

        let procedure = safe_unwrap_lock! {self, procedure}.clone();

        let errors = {
            let mut guard = safe_unwrap_lock! {self, error};
            std::mem::take(&mut *guard)
        };

        Ok(State {
            procedure,
            error: errors,
            scene_count: load_atomic! {self, scene_count},
            download_task_count: load_atomic! {self, download_task_count},
            purified_action_count: load_atomic! {self, purified_action_count},
            transpiled_action_count: load_atomic! {self, transpiled_action_count},
        })
    }
}

impl Drop for DefaultPipeline {
    fn drop(&mut self) {
        let _ = self.wait();
    }
}
