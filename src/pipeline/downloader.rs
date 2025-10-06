//! bestdori 资源下载

use std::collections::HashMap;
use std::fs::File;
use std::mem;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use super::definition::*;
use crate::constant::*;
use crate::error::*;

use futures_util::StreamExt;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use tokio::{io::AsyncWriteExt, runtime::Runtime, sync::Semaphore, time::timeout};

/// Bestdori 资源下载器
///
/// 说明：
/// - 非阻塞调度：所有下载任务通过内部线程与 tokio 运行时执行，调度操作对调用线程不阻塞。
/// - 有界并发：同时活跃的下载任务由 semaphore 限制（常量 DOWNLOAD_TASK_LIMIT）。队列本身无界。
/// - 支持绑定下载：使用 download_bind() 将对一个 URL 的字节回调为多个 Resource（回调在单独线程串行执行以保证同步回调安全）。
/// - 错误记录：下载过程中产生的错误会被收集到内部状态，通过 take_error() 提取并清空。
/// - 自动重试与超时：网络请求和写文件操作在每次尝试中有超时保护（常量 DOWNLOAD_TIMEOUT_SECS），并在遇到超时或暂时性错误时自动重试，重试次数由 DOWNLOAD_RETRY_TIMES 控制。
pub trait Downloader {
    /// 启动一个下载任务
    fn download(&mut self, resource: &Resource) -> Result<()>;

    /// 启动一个捆绑下载任务
    ///
    /// 获取 url 对应文件的字节, 传入回调函数生成资源列表.
    fn download_bind<F: BindTask>(&mut self, url: &str, task: F) -> Result<()>;

    /// 等待所有下载任务完成 (不关闭下载器)
    fn wait(&self) -> Result<()>;

    /// 中断下载并关闭下载器
    ///
    /// 如果工作线程已经终止，调用此方法不会报错；它尝试发送关闭命令并等待工作线程 join。
    fn shutdown(&mut self) -> Result<()>;

    /// 返回已记录的下载错误
    fn take_error(&mut self) -> Vec<DownloadError>;
}

// Type alias to simplify complex bind-queue type
type BindQueue = std::sync::Mutex<
    Vec<(
        Vec<u8>,
        Box<dyn Fn(Vec<u8>) -> Vec<Resource> + Send + 'static>,
    )>,
>;

/// 下载器配置
#[derive(Default, Clone, Deserialize)]
pub struct Header(HashMap<String, String>);

impl From<Header> for HeaderMap {
    fn from(value: Header) -> Self {
        let mut header_map = HeaderMap::new();

        for (key, value_str) in value.0 {
            let header_name = match HeaderName::from_bytes(key.as_bytes()) {
                Ok(name) => name,
                Err(_) => {
                    continue;
                }
            };
            let header_value = match HeaderValue::from_str(&value_str) {
                Ok(value) => value,
                Err(_) => {
                    continue;
                }
            };
            header_map.insert(header_name, header_value);
        }

        header_map
    }
}

/// 下载命令
enum DownloadCommand {
    Task {
        url: String,
        path: String,
    },
    Callback {
        url: String,
        cb: Box<dyn Fn(Vec<u8>) -> Vec<Resource> + Send + 'static>,
    },
    Shutdown,
}

/// 下载器内部状态
struct DownloaderState {
    task_count: usize,
    /// 当前正在执行初始 bundle 下载的数量 (Callback 下载)
    bind_active_count: usize,
    shutdown: bool,
    error: Vec<DownloadError>,
}

/// 默认 bestdori 资源下载器
pub struct DefaultDownloader {
    root: String,
    sender: mpsc::Sender<DownloadCommand>,
    handle: Option<thread::JoinHandle<()>>,
    state: Arc<(Mutex<DownloaderState>, Condvar)>,
    bind_queue: Arc<BindQueue>,
    bind_queue_len: Arc<AtomicUsize>,
    bind_notify: Arc<Condvar>,
}

impl DefaultDownloader {
    /// 创建一个新的下载器
    pub fn new(root: String) -> Result<Self> {
        Ok(Self::with_header(
            root,
            serde_json::from_reader(File::open_buffered(DOWNLOAD_HEADER)?)?,
        ))
    }

    /// 创建一个带配置的下载器
    pub fn with_header(root: String, header: Header) -> Self {
        // 创建命令通道
        let (sender, command_receiver) = mpsc::channel();

        // 并发下载许可（活跃下载上限）。队列与并发上限分离：队列无界，活跃并发由 semaphore 控制。
        let semaphore = std::sync::Arc::new(Semaphore::new(DOWNLOAD_TASK_LIMIT));

        // 创建共享状态
        let state = Arc::new((
            Mutex::new(DownloaderState {
                task_count: 0,
                bind_active_count: 0,
                shutdown: false,
                error: Vec::new(),
            }),
            Condvar::new(),
        ));

        // 克隆状态和 semaphore 用于工作线程
        let worker_state = state.clone();
        let worker_sema = semaphore.clone();

        // bind queue and notifier (shared between thread and async runtime)
        let bind_queue: Arc<BindQueue> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let bind_queue_len = Arc::new(AtomicUsize::new(0));
        let bind_notify = Arc::new(Condvar::new());

        // 创建工作线程（每个任务在 worker 中会根据 semaphore 控制并发）
        let worker_sender = sender.clone();
        let worker_root = root.clone();
        let bind_notify_clone = bind_notify.clone();
        let bind_queue_clone = bind_queue.clone();
        let bind_queue_len_clone = bind_queue_len.clone();
        let bind_notify_clone = bind_notify.clone();

        // 保存配置中的请求头
        let header = header.clone();

        let handle = thread::spawn(move || {
            // 创建工作线程的异步运行时
            let rt = Runtime::new().unwrap();

            // 启动 bind-processor（串行执行 bind 回调）
            {
                let sender_clone = worker_sender.clone();
                let root_clone = worker_root.clone();
                let bind_queue = bind_queue_clone.clone();
                let bind_queue_len = bind_queue_len_clone.clone();
                let bind_notify = bind_notify_clone.clone();
                let state_clone = worker_state.clone();

                // Use a dedicated std thread to serially process bind callbacks to avoid async/Send issues
                std::thread::spawn(move || {
                    loop {
                        // Wait until queue has items
                        let mut guard = bind_queue.lock().unwrap();
                        while guard.is_empty() {
                            // Wait on bind_notify condvar
                            guard = bind_notify.wait(guard).unwrap();
                        }

                        // pop one item (FIFO: pop from front)
                        let (bytes, cb) = guard.remove(0);
                        // decrease queue len
                        bind_queue_len.fetch_sub(1, Ordering::SeqCst);
                        drop(guard);

                        // mark bind_active_count++
                        {
                            let (lock, cvar) = &*state_clone;
                            let mut st = lock.lock().unwrap();
                            st.bind_active_count += 1;
                            cvar.notify_all();
                        }

                        // Execute callback (synchronous call) to produce resources
                        let resources = (cb)(bytes);

                        // enqueue produced resources as Tasks
                        for r in resources.into_iter() {
                            let _ = sender_clone.send(DownloadCommand::Task {
                                url: r.url.clone().unwrap_or_default(),
                                path: root_clone.clone() + r.get_full_path().as_str(),
                            });
                        }

                        // mark bind_active_count--
                        {
                            let (lock, cvar) = &*state_clone;
                            let mut st = lock.lock().unwrap();
                            if st.bind_active_count > 0 {
                                st.bind_active_count -= 1;
                            }
                            cvar.notify_all();
                        }
                    }
                });
            }

            // 运行工作循环
            Self::worker_loop(
                rt,
                command_receiver,
                worker_state,
                worker_sema,
                worker_sender,
                worker_root,
                bind_queue_clone,
                bind_queue_len_clone,
                bind_notify_clone,
                Some(header),
            );
        });

        Self {
            root,
            sender,
            handle: Some(handle),
            state,
            bind_queue,
            bind_queue_len,
            bind_notify,
        }
    }

    /// 工作线程主循环
    fn worker_loop(
        rt: Runtime,
        command_receiver: mpsc::Receiver<DownloadCommand>,
        state: Arc<(Mutex<DownloaderState>, Condvar)>,
        semaphore: std::sync::Arc<Semaphore>,
        sender: mpsc::Sender<DownloadCommand>,
        root: String,
        bind_queue: Arc<BindQueue>,
        bind_queue_len: Arc<AtomicUsize>,
        bind_notify: Arc<Condvar>,
        headers: Option<Header>,
    ) {
        let (state_lock, state_cvar) = &*state;

        // 创建带请求头的 Client
        let client = if let Some(header) = headers {
            Client::builder()
                .default_headers(header.into())
                .build()
                .unwrap_or_else(|_| Client::new())
        } else {
            Client::new()
        };

        // 迭代接收命令；发送端永不阻塞，worker 在内部根据 semaphore 控制活跃并发
        for command in command_receiver {
            match command {
                DownloadCommand::Task { url, path } => {
                    // 在异步任务中先获取 semaphore 许可，然后执行下载
                    let client = client.clone();
                    let state = state.clone();
                    let url_clone = url.clone();
                    let path_clone = path.clone();
                    let sema = semaphore.clone();

                    rt.spawn(async move {
                        // 获取并发许可（在此处 await，不会阻塞发送端）
                        let permit = sema.acquire_owned().await.unwrap();

                        // 增加活跃任务计数
                        {
                            let (lock, _cvar) = &*state;
                            let mut state_guard = lock.lock().unwrap();
                            state_guard.task_count += 1;
                        }

                        // 执行下载任务（download_resource 内部实现会对每次尝试做超时与重试）
                        let result: std::result::Result<(), DownloadError> =
                            Self::download_resource(&client, &url_clone, &path_clone).await;

                        // 记录可能出现的错误并减少计数；将 URL/path 上下文一并记录
                        let (lock, cvar) = &*state;
                        let mut state_guard = lock.lock().unwrap();
                        if let Err(mut derr) = result {
                            // 填充上下文（如果尚未设置）
                            if derr.url.is_none() {
                                derr.url = Some(url_clone.clone());
                            }
                            if derr.path.is_none() {
                                derr.path = Some(path_clone.clone());
                            }

                            state_guard.error.push(derr);
                        }

                        if state_guard.task_count > 0 {
                            state_guard.task_count -= 1;
                        }
                        cvar.notify_all();

                        // 释放并发许可（permit 在离开作用域时自动 drop）
                        drop(permit);
                    });
                }

                DownloadCommand::Callback { url, cb } => {
                    // 在异步任务中先获取 semaphore 许可，然后执行下载并将 bytes+cb 推入 bind_queue，由 bind-processor 串行处理回调
                    let client = client.clone();
                    let state = state.clone();
                    let url_clone = url.clone();
                    let sema = semaphore.clone();
                    let bind_queue = bind_queue.clone();
                    let bind_queue_len = bind_queue_len.clone();
                    let bind_notify = bind_notify.clone();

                    rt.spawn(async move {
                        let permit = sema.acquire_owned().await.unwrap();

                        // 标记为活跃任务
                        {
                            let (lock, _cvar) = &*state;
                            let mut state_guard = lock.lock().unwrap();
                            state_guard.task_count += 1;
                        }

                        // 执行获取字节的请求，带超时与重试
                        let mut maybe_bytes: Option<Vec<u8>> = None;
                        let mut maybe_error: Option<DownloadError> = None;

                        for _attempt in 0..DOWNLOAD_RETRY_TIMES {
                            // 把整个请求+读取 bytes 的过程放进 timeout 中
                            let attempt_res: std::result::Result<Vec<u8>, DownloadError> =
                                match timeout(
                                    Duration::from_secs(DOWNLOAD_TIMEOUT_SECS as u64),
                                    async {
                                        let resp = client.get(&url_clone).send().await?;
                                        if !resp.status().is_success() {
                                            return Err(DownloadErrorKind::HttpStatus(
                                                resp.status(),
                                            )
                                            .into());
                                        }
                                        let bytes = resp.bytes().await?;
                                        Ok(bytes.to_vec())
                                    },
                                )
                                .await
                                {
                                    Ok(Ok(bytes)) => Ok(bytes),
                                    Ok(Err(e)) => Err(e),
                                    Err(_) => Err(DownloadErrorKind::Timeout.into()),
                                };

                            match attempt_res {
                                Ok(bytes) => {
                                    maybe_bytes = Some(bytes);
                                    break;
                                }
                                Err(e) => {
                                    maybe_error = Some(e);
                                    // small backoff before retry
                                    tokio::time::sleep(Duration::from_millis(200)).await;
                                }
                            }
                        }

                        // 将可能的错误记录并减少 task_count
                        let (lock, cvar) = &*state;
                        let mut state_guard = lock.lock().unwrap();
                        if let Some(mut derr) = maybe_error {
                            if derr.url.is_none() {
                                derr.url = Some(url_clone.clone());
                            }
                            state_guard.error.push(derr);
                        }

                        if state_guard.task_count > 0 {
                            state_guard.task_count -= 1;
                        }
                        cvar.notify_all();

                        // 若成功获取 bytes，则将 (bytes, cb) 推入 bind_queue，由 bind-processor 串行处理
                        if let Some(bytes) = maybe_bytes {
                            {
                                let mut guard = bind_queue.lock().unwrap();
                                guard.push((bytes, cb));
                            }
                            bind_queue_len.fetch_add(1, Ordering::SeqCst);
                            bind_notify.notify_one();
                        }

                        drop(permit);
                    });
                }

                /* Lazy tasks removed: callers should either produce Resource and call download(),
                or use download_bind() to fetch bytes and produce Resource list. */
                DownloadCommand::Shutdown => {
                    // 标记关闭并等待在飞任务完成后再退出 worker
                    let mut state_guard = state_lock.lock().unwrap();
                    state_guard.shutdown = true;
                    while state_guard.task_count > 0 {
                        state_guard = state_cvar.wait(state_guard).unwrap();
                    }

                    break;
                }
            }
        }
    }

    /// 异步下载资源
    async fn download_resource(
        client: &Client,
        url: &str,
        path: &str,
    ) -> std::result::Result<(), DownloadError> {
        // 自动重试：在 DOWNLOAD_RETRY_TIMES 次尝试内处理超时/网络错误
        let mut last_err: Option<DownloadError> = None;
        for _attempt in 0..DOWNLOAD_RETRY_TIMES {
            // 每次尝试都在超时保护下执行完整的请求+写入流程
            let attempt = timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS as u64), async {
                let response = client.get(url).send().await?;
                if !response.status().is_success() {
                    return Err(DownloadErrorKind::HttpStatus(response.status()).into());
                }

                // 确保目标目录存在
                if let Some(parent) = Path::new(path).parent()
                    && !parent.as_os_str().is_empty()
                {
                    match tokio::fs::create_dir_all(parent).await {
                        Ok(_) => {}
                        Err(e) => return Err(e.into()),
                    }
                }

                // 创建目标文件并写入
                let mut file = tokio::fs::File::create(path).await?;
                let mut stream = response.bytes_stream();
                while let Some(chunk_res) = stream.next().await {
                    let chunk = chunk_res?;
                    file.write_all(&chunk).await?;
                }

                Ok(()) as std::result::Result<(), DownloadError>
            })
            .await;

            match attempt {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(e)) => last_err = Some(e),
                Err(_) => last_err = Some(DownloadErrorKind::Timeout.into()),
            }

            // 小的退避：避免立即重试打穿远端
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        Err(last_err.unwrap_or_else(|| DownloadErrorKind::Unexpected("unknown".into()).into()))
    }

    /// 获取当前任务数量
    fn task_count(&self) -> usize {
        let (lock, _) = &*self.state;
        let state_guard = lock.lock().unwrap();
        state_guard.task_count
    }
}

impl Downloader for DefaultDownloader {
    fn download(&mut self, resource: &Resource) -> Result<()> {
        // 检查URL是否存在
        if resource.url.is_none() {
            return Err(Error::Download(DownloadErrorKind::UrlMissing.into()));
        }

        // 非阻塞发送下载任务（避免阻塞调用线程）。当队列已满时返回 SendError。
        self.sender
            .send(DownloadCommand::Task {
                url: resource.url.clone().unwrap(),
                path: self.root.clone() + resource.get_full_path().as_str(),
            })
            .map_err(|e| {
                Error::Download(
                    DownloadErrorKind::SendError(format!("Failed to enqueue download task: {e}"))
                        .into(),
                )
            })
    }

    fn wait(&self) -> Result<()> {
        let (lock, cvar) = &*self.state;
        let mut state_guard = lock.lock().unwrap();

        // 等待直到任务数为0或下载器已关闭
        while state_guard.task_count > 0 && !state_guard.shutdown {
            state_guard = cvar.wait(state_guard).unwrap();
        }

        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        // 发送关闭命令；如果发送失败（通道已关闭），视为已经关闭，不当作错误返回
        let _ = self.sender.send(DownloadCommand::Shutdown);

        // 等待工作线程结束
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| Error::Download(DownloadErrorKind::WorkerPanic.into()))?;
        }

        Ok(())
    }

    fn take_error(&mut self) -> Vec<DownloadError> {
        let (lock, _) = &*self.state;
        let mut state_guard = lock.lock().unwrap();
        mem::take(&mut state_guard.error)
    }

    fn download_bind<F: BindTask>(&mut self, url: &str, task: F) -> Result<()> {
        // 将闭包装箱并发送 Callback 命令到 worker
        let boxed = Box::new(task);

        self.sender
            .send(DownloadCommand::Callback {
                url: url.to_string(),
                cb: boxed,
            })
            .map_err(|e| {
                Error::Download(
                    DownloadErrorKind::SendError(format!(
                        "Failed to enqueue download callback task: {e}"
                    ))
                    .into(),
                )
            })
    }
}

impl Drop for DefaultDownloader {
    fn drop(&mut self) {
        // let _ = self.wait();
        let _ = self.shutdown();
    }
}
