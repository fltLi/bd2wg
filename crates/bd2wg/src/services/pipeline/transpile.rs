//! 转译管线

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use reqwest::header::HeaderMap;

use crate::{
    error::*,
    false_or_panic, impl_drop_for_handle,
    models::{bestdori, webgal::Resource},
    services::{resolver::Resolver, transpiler::Transpiler},
    traits::{
        asset::Asset,
        handle::Handle,
        pipeline::{
            DownloadPipeline as DownloadPipelineTrait, TranspilePipeline as TranspilePipelineTrait,
            TranspileResult, TranspileState,
        },
        transpile::{self, Transpile},
    },
    utils::*,
};

use super::DownloadPipeline;

/// 转译管线
pub struct TranspilePipeline {
    cancel: Arc<AtomicBool>,
    state: Arc<RwLock<TranspileState>>,
    #[allow(clippy::type_complexity)]
    handle: Option<JoinHandle<(Vec<Error>, Vec<Arc<Resource>>)>>,

    root: PathBuf,
    header: Option<HeaderMap>, // 传递给下载管线
}

impl TranspilePipeline {
    /// 启动转译管线
    pub fn new(story: impl AsRef<Path>, root: impl AsRef<Path>, header: HeaderMap) -> Box<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let state: Arc<RwLock<TranspileState>> = Arc::default();

        let mut pipe = Box::new(Self {
            cancel: cancel.clone(),
            state: state.clone(),
            handle: None,
            root: root.as_ref().to_path_buf(),
            header: Some(header),
        });

        pipe.handle = Some({
            let story = story.as_ref().to_path_buf();
            let root = root.as_ref().to_path_buf();

            thread::spawn(move || Self::run(&story, &root, cancel, state))
        });

        // Self { handle: ..., ..pipe }
        pipe
    }

    /// 执行转译管线
    fn run(
        story: &Path, // Bestdori 脚本路径
        root: &Path,
        cancel: Arc<AtomicBool>,
        state: Arc<RwLock<TranspileState>>,
    ) -> (Vec<Error>, Vec<Arc<Resource>>) {
        macro_rules! unwrap_or_into_vec {
            ($expr:expr) => {
                match $expr {
                    Ok(v) => v,
                    Err(e) => return (vec![Error::File(e.into())], Vec::new()),
                }
            };
        }

        // 读取故事脚本
        let story = unwrap_or_into_vec! {
            bestdori::Story::from_bytes(
                &unwrap_or_into_vec! {fs::read(story)},
            )
        };

        false_or_panic! {cancel}

        // 执行转译
        let transpile::TranspileResult {
            story,
            resources,
            mut errors,
        } = Transpiler::<Resolver>::default().transpile(&story);

        false_or_panic! {cancel}

        {
            let (scene, action) = story.len();
            let mut state = state.write().unwrap();
            (state.scene, state.action) = (scene, action);
        }

        // 逐个写入场景
        for scene in story.iter() {
            false_or_panic! {cancel}

            if let Err(e) = create_and_write(scene.to_string(), &scene.absolute_path(root)) {
                errors.push(Error::File(e.into()));
            }
        }

        cancel.store(true, Ordering::Relaxed);
        (errors, resources)
    }
}

impl Handle for TranspilePipeline {
    type Result = (TranspileResult, Result<Box<dyn DownloadPipelineTrait>>);

    /// 等待转译管线结束
    ///
    /// panic: 转译管线被调用 cancel.
    fn join(mut self: Box<Self>) -> Self::Result {
        let (errors, res) = self.handle.take().unwrap().join().unwrap();
        let state = self.state.read().unwrap().clone();

        (
            TranspileResult { state, errors },
            DownloadPipeline::new(&self.root, self.header.take().unwrap(), res)
                .map(|pipe| -> Box<dyn DownloadPipelineTrait> { pipe }),
        )
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.handle = None;
    }

    fn is_finished(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

impl_drop_for_handle! {TranspilePipeline}

impl TranspilePipelineTrait for TranspilePipeline {
    fn state(&self) -> TranspileState {
        self.state.read().unwrap().clone()
    }
}
