//! 基础下载池实现

// TODO: 使用 crossbeam-channel 提供更优雅的管道实现.

// TODO: 使用 unstable mpmc 同时启动多个 DownloadPoolWorker.

use std::{
    collections::VecDeque,
    mem,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::{JoinHandle, sleep, spawn},
    time::Duration,
};

use bytes::Bytes;
use crossbeam_channel::{Receiver as MultiReceiver, Sender as MultiSender, unbounded};
use reqwest::{
    blocking::{Client, Response},
    header::HeaderMap,
};

use crate::{error::*, impl_drop_for_handle, traits::handle::Handle, utils::*};

/// 下载池返回类型
pub type PoolResult<T> = std::result::Result<T, DownloadErrorKind>;

/// 下载器工作线程计数
const CLIENT_COUNT: usize = 4;

/// 单个下载任务时间限制
const TASK_TIMEOUT: Duration = Duration::from_secs(16);

/// 单个下载任务最大重试次数
const TASK_MAX_RETRIES: usize = 3;

/// 客户端重启所需的连续失败次数
const CLIENT_RESTART_FAILURE_THRESHOLD: usize = 5;

/// 客户端重启等待时间
const CLIENT_RESTART_BACKOFF: Duration = Duration::from_secs(8);

/// 客户端连续重启在全部失败情况下的次数限制
const CLIENT_RESTART_LIMIT: usize = 3;

/// 下载命令
struct DownloadCommand {
    url: String,
    cancel: Arc<AtomicBool>,
    sender: Sender<PoolResult<Bytes>>,
}

/// 下载任务句柄
pub struct DownloadHandle {
    cancel: Arc<AtomicBool>,
    receiver: Receiver<PoolResult<Bytes>>,
}

impl Handle for DownloadHandle {
    type Result = PoolResult<Bytes>;

    /// 等待并获取下载结果
    ///
    /// panic: 下载池 / 句柄被调用 cancel.
    fn join(self: Box<Self>) -> Self::Result {
        self.receiver.recv().unwrap() // 下载池不应崩溃
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn is_finished(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

impl_drop_for_handle! {DownloadHandle}

/// 创建下载任务, 获取命令和句柄
fn new_download_task(url: &str) -> (DownloadCommand, Box<DownloadHandle>) {
    let cancel = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = channel();

    (
        DownloadCommand {
            url: url.to_string(),
            cancel: cancel.clone(),
            sender,
        },
        Box::new(DownloadHandle { cancel, receiver }),
    )
}

/// 下载任务
struct DownloadTask {
    count: usize,
    url: String,
    cancel: Arc<AtomicBool>,
    sender: Sender<PoolResult<Bytes>>,
}

impl DownloadTask {
    fn new(command: DownloadCommand) -> Self {
        let DownloadCommand {
            url,
            cancel,
            sender,
        } = command;

        Self {
            count: 0,
            url,
            cancel,
            sender,
        }
    }

    /// 提供返回值
    fn send(&mut self, res: PoolResult<Bytes>) {
        let _ = self.sender.send(res);
    }
}

impl Drop for DownloadTask {
    /// 更新结束标志
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// 下载池内部工作对象
///
/// 详细说明参考 run() 方法注释.
struct DownloadPoolWorker {
    count: usize,
    restart_count: usize,           // 连续全失败重启计数
    successes_since_restart: usize, // 自上次重启以来成功的任务数

    header: Arc<HeaderMap>, // 保存请求头以支持重新创建 Client
    client: Client,
    cancel: Arc<AtomicBool>,
    receiver: MultiReceiver<DownloadCommand>,
    tasks: VecDeque<DownloadTask>,
}

impl DownloadPoolWorker {
    /// 创建 (但不运行) 下载池内部管理
    fn new(
        header: Arc<HeaderMap>,
        cancel: Arc<AtomicBool>,
        receiver: MultiReceiver<DownloadCommand>,
    ) -> PoolResult<Self> {
        let client = new_client_with_header((*header).clone())?;

        Ok(Self {
            count: 0,
            restart_count: 0,
            successes_since_restart: 0,
            header,
            client,
            cancel: cancel.clone(),
            receiver,
            tasks: VecDeque::new(),
        })
    }

    /// 退出全部下载任务
    fn cancel(&mut self) {
        drop(mem::take(&mut self.tasks));
    }

    /// 接收并启动一些下载任务
    fn receive(&mut self) {
        if !self.tasks.is_empty() {
            // 有任务时, 非阻塞获取并加入一个任务
            if let Ok(cmd) = self.receiver.try_recv() {
                self.tasks.push_back(DownloadTask::new(cmd));
            }
        } else if let Ok(cmd) = self.receiver.recv() {
            // 没有任务时, 阻塞等待下一个任务
            // 当 Sender 丢弃时, 忽略错误, run() 将进入下一轮开头的退出检查分支
            self.tasks.push_back(DownloadTask::new(cmd));
        }
    }

    // ---------------- task: begin ----------------

    /// 处理单个下载任务 (从队列中弹出后调用)
    fn handle_task(&mut self, task: DownloadTask) {
        // 检查取消
        if task.cancel.load(Ordering::Relaxed) {
            return;
        }
        // 尝试下载 (阻塞)
        let res = self.client.get(&task.url).timeout(TASK_TIMEOUT).send();

        // 处理响应
        self.handle_response(task, res);

        // 若连续失败次数超过阈值, 尝试重启 client
        if self.count >= CLIENT_RESTART_FAILURE_THRESHOLD {
            // 根据自上次重启以来是否有成功, 更新连续全失败重启计数
            if self.successes_since_restart == 0 {
                self.restart_count = self.restart_count.saturating_add(1);
            } else {
                self.restart_count = 0;
            }
            // 重启后清零成功计数, 准备记录下一轮
            self.successes_since_restart = 0;

            // 等待一段时间再尝试重建 client
            sleep(CLIENT_RESTART_BACKOFF);
            if let Ok(client) = new_client_with_header((*self.header).clone()) {
                self.client = client;
            }
            // 清空连续失败计数
            self.count = 0;
        }
    }

    /// 处理 `send()` 的返回值分支 (主入口)
    fn handle_response(
        &mut self,
        task: DownloadTask,
        res: std::result::Result<Response, reqwest::Error>,
    ) {
        match res {
            Ok(resp) => self.handle_response_ok(task, resp),
            Err(e) => self.handle_request_error(task, e),
        }
    }

    /// 处理成功返回的 Response
    fn handle_response_ok(&mut self, mut task: DownloadTask, resp: reqwest::blocking::Response) {
        match resp.error_for_status() {
            Ok(resp) => {
                // 检查 Content-Encoding, 在 reqwest 未自动解压的情况下提供回退解码
                let encoding = resp
                    .headers()
                    .get(reqwest::header::CONTENT_ENCODING)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();

                match resp.bytes() {
                    Ok(bytes) => match maybe_decompress_bytes(&bytes, &encoding) {
                        Ok(out) => self.handle_success(task, Bytes::from(out)),
                        Err(e) => task.send(Err(DownloadErrorKind::Io(e))),
                    },
                    Err(e) => self.handle_body_error(task, e),
                }
            }

            // 将非 2xx 的 HTTP 状态视为请求错误, 交由请求错误分支处理并重试
            Err(e) => self.handle_request_error(task, e),
        }
    }

    /// 请求成功且读取 body 成功
    fn handle_success(&mut self, mut task: DownloadTask, bytes: Bytes) {
        self.count = 0;
        self.restart_count = 0;
        self.successes_since_restart = self.successes_since_restart.saturating_add(1);
        task.send(Ok(bytes));
    }

    /// 请求成功但读取 body 出错
    fn handle_body_error(&mut self, task: DownloadTask, err: reqwest::Error) {
        self.increment_failure_and_maybe_retry(task, err);
    }

    /// 请求发起阶段出错 (包含超时)
    fn handle_request_error(&mut self, task: DownloadTask, err: reqwest::Error) {
        self.increment_failure_and_maybe_retry(task, err);
    }

    /// 增加失败计数并决定是重试还是结束任务
    fn increment_failure_and_maybe_retry(&mut self, mut task: DownloadTask, err: reqwest::Error) {
        task.count += 1;
        self.count += 1;
        if task.count >= TASK_MAX_RETRIES || self.restart_count >= CLIENT_RESTART_LIMIT {
            task.send(Err(DownloadErrorKind::Reqwest(err)));
        } else {
            self.tasks.push_back(task);
        }
    }

    // ----------------- task: end -----------------

    /// (阻塞) 执行主循环
    ///
    /// 保证下载循环不会崩溃, 进而保证下载任务和下载池句柄的有效性.
    ///
    /// 每次循环时, 检查下载池和下载任务的退出信号, 然后尝试处理最早的任务.
    ///
    /// 错误处理:
    /// 1. 下载任务超时 / 出错时, 先推入队尾重新尝试.
    /// 2. 单个任务多次失败, 该任务结束并返回最后一次错误信息.
    /// 3. 连续多个任务失败, 将在一段时间后启动新的 client, 并清空任务的错误计数.  
    ///    连续多次重启失败 / 没有任务成功将清空队列中的任务.
    fn run(mut self) {
        loop {
            // 检查退出
            if self.cancel.load(Ordering::Relaxed) {
                self.cancel();
                break;
            }

            // 接收任务
            self.receive();

            // 处理任务
            if let Some(task) = self.tasks.pop_front() {
                self.handle_task(task);
            }
        }
    }
}

/// 下载池
///
/// 简单, 一定程度稳健的轻量级下载器.
///
/// 持有独立运行的子线程, 内部阻塞地执行下载任务.
/// 下载任务超时时推入队尾稍后重试, 多次重试报错.
#[derive(Debug)]
pub struct DownloadPool {
    cancel: Arc<AtomicBool>,
    sender: MultiSender<DownloadCommand>,
    handles: Vec<JoinHandle<()>>,
}

impl DownloadPool {
    /// 根据请求头启动下载池
    pub fn new(header: HeaderMap) -> PoolResult<Box<Self>> {
        let header = Arc::new(header);
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = unbounded();

        // 同时启动多个工作线程
        let handles = (0..CLIENT_COUNT)
            .map(|_| {
                let worker =
                    DownloadPoolWorker::new(header.clone(), cancel.clone(), receiver.clone())?;
                Ok(spawn(move || worker.run()))
            })
            .collect::<PoolResult<_>>()?;

        Ok(Box::new(Self {
            handles,
            cancel,
            sender,
        }))
    }

    /// 创建下载任务
    ///
    /// 非阻塞地在子线程启动下载任务, 返回任务句柄.
    ///
    /// panic: 下载池被调用 cancel.
    pub fn download(&mut self, url: &str) -> Box<DownloadHandle> {
        #[cfg(debug_assertions)]
        dbg!(url);

        let (cmd, handle) = new_download_task(url);
        self.sender.send(cmd).unwrap();
        handle
    }
}

impl Handle for DownloadPool {
    type Result = ();

    /// 等待下载任务完成
    ///
    /// panic: 下载池被调用 cancel.
    fn join(mut self: Box<Self>) -> Self::Result {
        for handle in mem::take(&mut self.handles) {
            handle.join().unwrap(); // 下载池不应崩溃
        }
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.handles.clear();
    }

    /// 询问下载任务是否均已完成
    fn is_finished(&self) -> bool {
        self.handles.iter().any(|handle| handle.is_finished())
    }
}

impl_drop_for_handle! {DownloadPool}
