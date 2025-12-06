//! Bestdori 下载器

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use std::{fs, thread};

use reqwest::header::HeaderMap;

use crate::models::bestdori;
use crate::models::webgal::{self, Resource, ResourceType, default_model_config_path};
use crate::services::downloader::pool;
use crate::traits::asset::Asset;
use crate::traits::handle;
use crate::traits::{downloader::Downloader as DownloaderTrait, handle::Handle};
use crate::utils::create_and_write;
use crate::{error::*, impl_drop_for_handle};

use super::pool::{DownloadHandle, DownloadPool};

/// Downloader join(): Live2d 任务结束状态检查间隔时间
const DOWNLOAD_JOIN_CHECK_BACKOFF: Duration = Duration::from_secs(1);

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
    /// panic: 下载器 / 句柄被调用 cancel.
    fn join(mut self) -> Self::Result {
        self.handle
            .take()
            .unwrap()
            .join()
            .and_then(|bytes| create_and_write(&bytes, &self.path).map_err(DownloadErrorKind::Io))
            .map_err(|error| {
                Error::Download(DownloadError {
                    url: self.url.clone(),
                    path: self.path.clone(),
                    error,
                })
            })
    }

    fn cancel(&mut self) {
        if let Some(mut handle) = self.handle.take() {
            handle.cancel();
        }
    }

    fn is_finished(&self) -> bool {
        self.handle
            .as_ref()
            .is_none_or(|handle| handle.is_finished())
    }
}

impl_drop_for_handle! {CommonDownloadHandle}

struct Live2dDownloadWorker {
    url: String,
    path: PathBuf, // Live2D 资源根目录
    cancel: Arc<AtomicBool>,
    count: Arc<AtomicUsize>,
    pool: Arc<Mutex<DownloadPool>>,
    sender: Sender<Result<()>>,
}

impl Live2dDownloadWorker {
    /// 创建新下载任务 (不立即执行)
    fn new(
        url: &str,
        path: &Path,
        count: Arc<AtomicUsize>,
        pool: Arc<Mutex<DownloadPool>>,
    ) -> (Self, Arc<AtomicBool>, Receiver<Result<()>>) {
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = channel();
        (
            Self {
                url: url.to_string(),
                path: path.to_path_buf(),
                cancel: cancel.clone(),
                count,
                pool,
                sender,
            },
            cancel,
            receiver,
        )
    }

    /// 结束任务, 提供返回值
    fn send(self, result: Result<()>) {
        let _ = self.sender.send(result);
    }

    /// (阻塞) 执行主循环
    fn run(self) {
        // 生成下载错误
        let download_error = |error| {
            Error::Download(DownloadError {
                url: self.url.clone(),
                path: self.path.clone(),
                error,
            })
        };

        /// 检查结果, 失败时退出
        macro_rules! unwrap_or_exit {
            ($result:expr) => {
                match $result {
                    Ok(v) => v,
                    Err(e) => {
                        self.send(Err(e));
                        return;
                    }
                }
            };
        }

        // 获取 Live2D 配置
        let handle = self.pool.lock().unwrap().download(&self.url);
        let resource = unwrap_or_exit! {
            handle
            .join()
            .map_err(download_error)
            .and_then(|model| bestdori::Model::from_slice(&model))
            .and_then(|model| {
                // 解析为 WebGAL Live2D 配置文件
                let (model, resource) = webgal::Model::from_bestdori_model(model);

                // 写入配置文件
                create_and_write(
                    &serde_json::to_vec_pretty(&model).map_err(Error::SerdeJson)?,
                    Path::new(&default_model_config_path(&self.path.to_string_lossy())),
                )
                .map_err(|err| download_error(err.into()))
                .map(|_| resource)
            })
        };

        // 下载相关资源
        for (url, path) in resource {
            // 检查退出
            if self.cancel.load(Ordering::Relaxed) {
                return;
            }

            // 下载资源
            let handle = self.pool.lock().unwrap().download(&url);
            unwrap_or_exit! {
                handle.join().map_err(download_error).and_then(|bytes| {
                    create_and_write(&bytes, &path).map_err(|err| download_error(err.into()))
                })
            };
        }
    }
}

impl Drop for Live2dDownloadWorker {
    /// 更改相应原子量
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::Relaxed);
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Live2D 下载任务句柄
struct Live2dDownloadHandle {
    handle: Option<JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Result<()>>,
}

impl Live2dDownloadHandle {
    /// 创建 Live2D 下载任务
    fn new(
        url: &str,
        path: &Path,
        count: Arc<AtomicUsize>,
        pool: Arc<Mutex<DownloadPool>>,
    ) -> Self {
        let (worker, cancel, receiver) = Live2dDownloadWorker::new(url, path, count, pool);
        let handle = thread::spawn(move || worker.run());
        Self {
            handle: Some(handle),
            cancel,
            receiver,
        }
    }
}

impl Handle for Live2dDownloadHandle {
    type Result = Result<()>;

    fn join(mut self) -> Self::Result {
        let _ = self.handle.take().unwrap().join();
        Ok(())
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn is_finished(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

impl_drop_for_handle! {Live2dDownloadHandle}

/// Bestdori 下载器
///
/// 根据不同的资源类型下载对应资源
pub struct Downloader {
    root: PathBuf,
    count: Arc<AtomicUsize>, // Live2D 任务计数
    pool: Option<Arc<Mutex<DownloadPool>>>,
}

impl Downloader {
    /// 在指定目录创建下载器
    fn new<P: AsRef<Path>>(root: P, headers: HeaderMap) -> Result<Self> {
        Ok(Self {
            root: root.as_ref().to_path_buf(),
            count: Arc::new(AtomicUsize::new(0)),
            pool: Some(Arc::new(Mutex::new(
                DownloadPool::new(headers).map_err(DownloadError::from)?,
            ))),
        })
    }

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
    ///
    /// resource.url 实际为 buildScript url.
    fn download_model(&mut self, resource: &Resource) -> Live2dDownloadHandle {
        Live2dDownloadHandle::new(
            &resource.url,
            &resource.absolute_path(&self.root), // 编译器会优化掉 & + clone 吧...
            self.count.clone(),
            self.pool.as_ref().unwrap().clone(),
        )
    }
}

impl Handle for Downloader {
    type Result = ();

    /// 等待下载任务完成并返回
    ///
    /// panic: 下载器被调用 cancel.
    fn join(mut self) -> Self::Result {
        // 等待 Live2D 下载任务
        while self.count.load(Ordering::Relaxed) != 0 {
            thread::sleep(DOWNLOAD_JOIN_CHECK_BACKOFF);
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
            .is_none_or(|pool| pool.lock().unwrap().is_finished())
    }
}

impl DownloaderTrait for Downloader {
    fn download<R: AsRef<Resource>>(
        &mut self,
        resource: R,
    ) -> Box<dyn Handle<Result = Result<()>>> {
        let resource = resource.as_ref();
        match resource.kind {
            ResourceType::Figure => Box::new(self.download_model(resource)),
            _ => Box::new(self.download_normal(resource)),
        }
    }
}

impl_drop_for_handle! {Downloader}
