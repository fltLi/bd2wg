//! Bestdori 下载器

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use std::{fs, thread};

use crate::models::webgal::{Resource, ResourceType};
use crate::services::downloader::pool;
use crate::traits::asset::Asset;
use crate::traits::handle;
use crate::traits::{downloader::Downloader as DownloaderTrait, handle::Handle};
use crate::{error::*, impl_drop_for_handle};

use super::pool::{DownloadHandle, DownloadPool};

const JOIN_CHECK_BACKOFF: Duration = Duration::from_secs(1);

/// 常规下载任务句柄
struct CommonDownloadHandle {
    url: String,
    path: PathBuf,
    handle: Option<DownloadHandle>,
}

impl Handle for CommonDownloadHandle {
    type Result = Result<()>;

    /// 等待下载任务完成
    ///
    /// 若此操作前已调用 cancel, 将发生 panic.
    fn join(mut self) -> Self::Result {
        let url = self.url.clone();
        let path = self.path.clone();
        let handle = self.handle.take().unwrap();

        let new_error = |error| DownloadError {
            url: url.clone(),
            path: path.clone(),
            error,
        };

        let bytes = handle.join().map_err(new_error)?;

        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|err| new_error(err.into()))?;
        }
        fs::write(&path, &bytes).map_err(|err| new_error(err.into()))?;

        Ok(())
    }

    fn cancel(&mut self) {
        if let Some(mut handle) = self.handle.take() {
            handle.cancel();
        }
    }

    fn is_finished(&self) -> bool {
        self.handle
            .as_ref()
            .map_or(true, |handle| handle.is_finished())
    }
}

impl_drop_for_handle! {CommonDownloadHandle}

/// Live2D 下载任务句柄
struct Live2dDownloadHandle {
    url: String,
    path: PathBuf, // Live2D 资源根目录
    handle: JoinHandle<()>,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<super::pool::Result<()>>,
}

impl Handle for Live2dDownloadHandle {
    type Result = Result<()>;

    // TODO: Live2dDownloadHandle 的 Handle 实现.
}

impl_drop_for_handle! {Live2dDownloadHandle}

/// Bestdori 下载器
///
/// 根据不同的资源类型下载对应资源
pub struct Downloader {
    root: PathBuf,
    pool: Option<Arc<Mutex<DownloadPool>>>,
}

impl Downloader {
    /// 下载普通资源
    fn download_normal(&mut self, resource: &Resource) -> CommonDownloadHandle {
        let path = resource.absolute_path(&self.root);
        let handle = self
            .pool
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .download(&resource.url);

        CommonDownloadHandle {
            url: resource.url.clone(),
            path,
            handle: Some(handle),
        }
    }

    /// 下载 Live2D 模型
    fn download_model(&mut self, resource: &Resource) -> Live2dDownloadHandle {
        unimplemented!("TODO: Live2dDownloadHandle::new()") // TODO
    }
}

impl Handle for Downloader {
    type Result = ();

    /// 等待下载任务完成并返回
    ///
    /// 若此操作前已调用 cancel, 将发生 panic.
    fn join(mut self) -> Self::Result {
        // 等待 Live2D 下载任务
        while Arc::strong_count(&self.pool.as_ref().unwrap()) != 1 {
            thread::sleep(JOIN_CHECK_BACKOFF);
        }

        // 等待常规下载任务
        Arc::try_unwrap(self.pool.take().unwrap())
            .unwrap()
            .into_inner()
            .unwrap()
            .join();
    }

    fn cancel(&mut self) {
        // 子线程中的 Live2dDownloadHandle 会自然 panic.
        if let Some(pool) = self.pool.take() {
            pool.lock().unwrap().cancel();
        }
    }

    fn is_finished(&self) -> bool {
        self.pool
            .as_ref()
            .map_or(true, |pool| pool.lock().unwrap().is_finished())
    }
}

impl DownloaderTrait for Downloader {
    fn download<R: AsRef<Resource>>(
        &mut self,
        resource: R,
    ) -> Result<Box<dyn Handle<Result = Result<()>>>> {
        let resource = resource.as_ref();
        match resource.kind {
            ResourceType::Figure => Box::new(self.download_model(resource)),
            _ => Box::new(self.download_normal(resource)),
        }
    }
}

impl_drop_for_handle! {Downloader}
