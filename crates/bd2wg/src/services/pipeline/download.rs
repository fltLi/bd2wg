//! 下载管线

use std::{
    path::Path,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle, sleep},
    time::Duration,
};

use reqwest::header::HeaderMap;

use crate::{
    error::*,
    false_or_panic, impl_drop_for_handle,
    models::webgal::Resource,
    services::downloader::Downloader,
    traits::{
        download::Download,
        handle::Handle,
        pipeline::{DownloadPipeline as DownloadPipelineTrait, DownloadResult, DownloadState},
    },
};

/// 下载状态更新间隔
const DOWNLOAD_STATE_UPDATE_BACKOFF: Duration = Duration::from_millis(100);

/// 下载管线
pub struct DownloadPipeline {
    cancel: Arc<AtomicBool>,
    state: Arc<RwLock<DownloadState>>,
    handle: Option<JoinHandle<Vec<Error>>>,
}

impl DownloadPipeline {
    /// 启动下载管线
    pub fn new(
        root: impl AsRef<Path>,
        header: HeaderMap,
        res: Vec<Arc<Resource>>,
    ) -> Result<Box<Self>> {
        let downloader = Downloader::new(root, header)?;

        let cancel = Arc::new(AtomicBool::new(false));
        let state = Arc::new(RwLock::new(DownloadState {
            total: res.len(),
            ..Default::default()
        }));

        let mut pipe = Box::new(Self {
            cancel: cancel.clone(),
            state: state.clone(),
            handle: None,
        });

        pipe.handle = Some(thread::spawn(move || {
            Self::run(downloader, res, cancel, state)
        }));

        Ok(pipe)
    }

    /// 执行下载管线
    fn run(
        mut downloader: Downloader,
        resources: Vec<Arc<Resource>>,
        cancel: Arc<AtomicBool>,
        state: Arc<RwLock<DownloadState>>,
    ) -> Vec<Error> {
        let mut errors = Vec::new();

        // 启动下载任务
        let mut handles: Vec<_> = resources
            .into_iter()
            .map(|res| downloader.download(res))
            .collect();

        // 状态检查
        let mut check = || -> bool {
            if handles.is_empty() {
                return false;
            }

            // 检查已完成的任务
            let done: Vec<_> = handles
                .iter()
                .enumerate()
                .filter_map(|(k, task)| if task.is_finished() { Some(k) } else { None })
                .collect();

            let mut success = 0;
            let mut failed = 0;

            // 清理任务
            for k in done.into_iter().rev() {
                let task = handles.swap_remove(k);

                match task.join() {
                    Ok(_) => success += 1,
                    Err(mut e) => {
                        failed += 1;
                        errors.append(&mut e);
                    }
                }
            }

            // 更新计数
            state.write().unwrap().success += success;
            state.write().unwrap().failed += failed;

            true
        };

        // 监听循环
        // while !check() {  // 耻辱柱!
        while check() {
            false_or_panic! {cancel}

            sleep(DOWNLOAD_STATE_UPDATE_BACKOFF);
        }

        cancel.store(true, Ordering::Relaxed);
        errors
    }
}

impl Handle for DownloadPipeline {
    type Result = DownloadResult;

    /// 等待下载管线结束
    ///
    /// panic: 下载管线被调用 cancel.
    fn join(mut self: Box<Self>) -> Self::Result {
        let state = self.state.read().unwrap().clone();
        let errors = self.handle.take().unwrap().join().unwrap();

        DownloadResult { state, errors }
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.handle = None;
    }

    fn is_finished(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

impl_drop_for_handle! {DownloadPipeline}

impl DownloadPipelineTrait for DownloadPipeline {
    fn state(&self) -> DownloadState {
        self.state.read().unwrap().clone()
    }
}
