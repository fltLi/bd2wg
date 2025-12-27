//! Bestdori 下载器

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle, sleep},
    time::Duration,
};

use reqwest::header::HeaderMap;

use crate::{
    error::*,
    false_or_panic, impl_drop_for_handle,
    models::{
        bestdori,
        webgal::{self, Resource, ResourceType, default_model_config_path},
    },
    traits::{asset::Asset, download::Download, handle::Handle},
    utils::*,
};

use super::pool::{DownloadHandle, DownloadPool};

type DownloadResult = std::result::Result<(), Vec<Error>>;

/// Downloader join(): Live2d 任务结束状态检查间隔时间
const DOWNLOAD_JOIN_CHECK_BACKOFF: Duration = Duration::from_secs(1);

/// 常规下载任务句柄
struct CommonDownloadHandle {
    url: String,
    path: PathBuf,
    handle: Option<Box<DownloadHandle>>,
}

impl Handle for CommonDownloadHandle {
    type Result = DownloadResult;

    /// 等待下载任务完成
    ///
    /// panic: 下载器 / 句柄被调用 cancel.
    fn join(mut self: Box<Self>) -> Self::Result {
        self.handle
            .take()
            .unwrap()
            .join()
            .and_then(|bytes| create_and_write(&bytes, &self.path).map_err(DownloadErrorKind::Io))
            .map_err(|e| {
                vec![Error::Download(DownloadError {
                    url: self.url.clone(),
                    path: self.path.clone(),
                    error: e,
                })]
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
    pool: Arc<Mutex<Box<DownloadPool>>>,
}

impl Live2dDownloadWorker {
    /// 创建新下载任务 (不立即执行)
    fn new(
        url: &str,
        path: &Path,
        count: Arc<AtomicUsize>,
        pool: Arc<Mutex<Box<DownloadPool>>>,
    ) -> (Self, Arc<AtomicBool>) {
        let cancel = Arc::new(AtomicBool::new(false));

        count.fetch_add(1, Ordering::Relaxed);

        (
            Self {
                url: url.to_string(),
                path: path.to_path_buf(),
                cancel: cancel.clone(),
                count,
                pool,
            },
            cancel,
        )
    }

    /// (阻塞) 执行主循环
    fn run(self) -> DownloadResult {
        // 生成下载错误
        let download_error = |error| {
            Error::Download(DownloadError {
                url: self.url.clone(),
                path: self.path.clone(),
                error,
            })
        };

        // 获取 Live2D 配置
        let handle = self.pool.lock().unwrap().download(&self.url);
        let resource = handle
            .join()
            .map_err(download_error)
            // 解析 Bestdori Live2D 配置文件
            .and_then(|model| {
                bestdori::Model::from_slice(&model).map_err(|e| download_error(e.into()))
            })
            .and_then(|model| {
                // 解析为 WebGAL Live2D 配置文件
                let (model, res) = webgal::Model::from_bestdori_model(model);

                // 写入配置文件
                create_and_write(
                    &serde_json::to_vec_pretty(&model).map_err(|e| download_error(e.into()))?,
                    Path::new(&default_model_config_path(&self.path.to_string_lossy())),
                )
                .map_err(|e| download_error(e.into()))?;

                // 合成完整路径
                Ok(res
                    .into_iter()
                    .map(|(url, path)| (url, self.path.join(path))))
            })
            .map_err(|e| vec![e])?;

        // 启动下载
        let handles = resource
            .into_iter()
            .map(|(url, path)| (self.pool.lock().unwrap().download(&url), path));

        // 等待并处理下载结果
        let errors: Vec<_> = handles
            .into_iter()
            .filter_map(|(handle, path)| {
                false_or_panic! {self.cancel}

                handle
                    .join()
                    .map_err(download_error)
                    .and_then(|bytes| {
                        // 写入本地文件
                        create_and_write(&bytes, &path).map_err(|err| download_error(err.into()))
                    })
                    .err() // 保留失败错误
            })
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
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
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<DownloadResult>>,
}

impl Live2dDownloadHandle {
    /// 创建 Live2D 下载任务
    fn new(
        url: &str,
        path: &Path,
        count: Arc<AtomicUsize>,
        pool: Arc<Mutex<Box<DownloadPool>>>,
    ) -> Box<Self> {
        let (worker, cancel) = Live2dDownloadWorker::new(url, path, count, pool);
        let handle = thread::spawn(move || worker.run());

        Box::new(Self {
            cancel,
            handle: Some(handle),
        })
    }
}

impl Handle for Live2dDownloadHandle {
    type Result = DownloadResult;

    fn join(mut self: Box<Self>) -> Self::Result {
        self.handle.take().unwrap().join().unwrap()
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.handle = None;
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
    pool: Option<Arc<Mutex<Box<DownloadPool>>>>,
}

impl Downloader {
    /// 在指定目录创建下载器
    pub fn new(root: impl AsRef<Path>, header: HeaderMap) -> Result<Self> {
        Ok(Self {
            root: root.as_ref().to_path_buf(),
            count: Arc::new(AtomicUsize::new(0)),
            pool: Some(Arc::new(Mutex::new(
                DownloadPool::new(header).map_err(DownloadError::from)?,
            ))),
        })
    }

    /// 下载普通资源
    fn download_normal(&mut self, res: &Resource) -> Box<CommonDownloadHandle> {
        let path = res.absolute_path(&self.root);
        let handle = self
            .pool
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .download(&res.url);

        Box::new(CommonDownloadHandle {
            url: res.url.clone(),
            path,
            handle: Some(handle),
        })
    }

    /// 下载 Live2D 模型
    ///
    /// resource.url 实际为 buildScript url.
    fn download_model(&mut self, res: &Resource) -> Box<Live2dDownloadHandle> {
        Live2dDownloadHandle::new(
            &res.url,
            &res.absolute_path(&self.root), // 编译器会优化掉 & + clone 吧...
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
    fn join(mut self: Box<Self>) -> Self::Result {
        // 等待 Live2D 下载任务
        while self.count.load(Ordering::Relaxed) != 0 {
            sleep(DOWNLOAD_JOIN_CHECK_BACKOFF);
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

impl Download for Downloader {
    fn download(
        &mut self,
        res: impl AsRef<Resource>,
    ) -> Box<dyn Handle<Result = std::result::Result<(), Vec<Error>>>> {
        let res = res.as_ref();
        match res.kind {
            ResourceType::Figure => self.download_model(res),
            _ => self.download_normal(res),
        }
    }
}

impl_drop_for_handle! {Downloader}
