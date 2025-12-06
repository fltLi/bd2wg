//! 基础下载池实现

// TODO: 使用 crossbeam-channel 提供更优雅的管道实现.

use std::collections::VecDeque;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use bytes::Bytes;
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;

use crate::error::*;
use crate::impl_drop_for_handle;
use crate::traits::handle::Handle;
use crate::utils::new_client_with_headers;

/// 下载池返回类型
pub type Result<T> = std::result::Result<T, DownloadErrorKind>;

/// 单个下载任务时间限制
const DOWNLOAD_TASK_TIMEOUT: Duration = Duration::from_secs(16);

/// 单个下载任务最大重试次数
const DOWNLOAD_TASK_MAX_RETRIES: usize = 3;

/// 客户端重启所需的连续失败次数
const CLIENT_RESTART_FAILURE_THRESHOLD: usize = 5;

/// 客户端重启等待时间
const CLIENT_RESTART_BACKOFF: Duration = Duration::from_secs(8);

/// 下载命令
struct DownloadCommand {
    url: String,
    cancel: Arc<AtomicBool>,
    sender: Sender<Result<Bytes>>,
}

/// 下载任务句柄
pub struct DownloadHandle {
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Result<Bytes>>,
}

impl Handle for DownloadHandle {
    type Result = Result<Bytes>;

    /// 等待并获取下载结果
    ///
    /// panic: 下载池 / 句柄被调用 cancel.
    fn join(self) -> Self::Result {
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
fn new_download_task(url: &str) -> (DownloadCommand, DownloadHandle) {
    let cancel = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = channel();

    (
        DownloadCommand {
            url: url.to_string(),
            cancel: cancel.clone(),
            sender,
        },
        DownloadHandle { cancel, receiver },
    )
}

/// 下载任务
struct DownloadTask {
    count: usize,
    url: String,
    cancel: Arc<AtomicBool>,
    sender: Sender<Result<Bytes>>,
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
    fn send(&mut self, res: Result<Bytes>) {
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
    headers: HeaderMap, // 保存请求头以支持重新创建 Client
    client: Client,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<DownloadCommand>,
    tasks: VecDeque<DownloadTask>,
}

impl DownloadPoolWorker {
    /// 创建 (但不运行) 下载池内部管理
    fn new(headers: HeaderMap) -> Result<(Self, Arc<AtomicBool>, Sender<DownloadCommand>)> {
        let client = new_client_with_headers(headers.clone())?;

        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = channel();

        Ok((
            Self {
                count: 0,
                headers,
                client,
                cancel: cancel.clone(),
                receiver,
                tasks: VecDeque::new(),
            },
            cancel,
            sender,
        ))
    }

    /// 退出全部下载任务
    fn cancel(&mut self) {
        drop(mem::take(&mut self.tasks));
    }

    /// 接收并启动一些下载任务
    fn receive(&mut self) {
        if !self.tasks.is_empty() {
            // 有任务时, 非阻塞加入当前全部任务
            self.receiver.try_iter().for_each(|cmd| {
                self.tasks.push_back(DownloadTask::new(cmd));
            });
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
        let res = self
            .client
            .get(&task.url)
            .timeout(DOWNLOAD_TASK_TIMEOUT)
            .send();

        // 处理响应
        self.handle_response(task, res);

        // 若连续失败次数超过阈值，尝试重启 client
        if self.count >= CLIENT_RESTART_FAILURE_THRESHOLD {
            // 等待一段时间再尝试重建 client
            thread::sleep(CLIENT_RESTART_BACKOFF);
            if let Ok(client) = new_client_with_headers(self.headers.clone()) {
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
        res: std::result::Result<reqwest::blocking::Response, reqwest::Error>,
    ) {
        match res {
            Ok(resp) => self.handle_response_ok(task, resp),
            Err(e) => self.handle_request_error(task, e),
        }
    }

    /// 处理成功返回的 Response
    fn handle_response_ok(&mut self, task: DownloadTask, resp: reqwest::blocking::Response) {
        match resp.bytes() {
            Ok(bytes) => self.handle_success(task, bytes),
            Err(e) => self.handle_body_error(task, e),
        }
    }

    /// 请求成功且读取 body 成功
    fn handle_success(&mut self, mut task: DownloadTask, bytes: Bytes) {
        self.count = 0;
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
        if task.count >= DOWNLOAD_TASK_MAX_RETRIES {
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
    ///    考虑到场景的简单性, 以及为了保证线程的有效性, 多次重启失败仍将**继续执行下载**.
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
    handle: Option<JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
    sender: Sender<DownloadCommand>,
}

impl DownloadPool {
    /// 根据请求头启动下载池
    pub fn new(headers: HeaderMap) -> Result<Self> {
        let (worker, cancel, sender) = DownloadPoolWorker::new(headers)?;
        let handle = thread::spawn(move || worker.run());

        Ok(Self {
            handle: Some(handle),
            cancel,
            sender,
        })
    }

    /// 创建下载任务
    ///
    /// 非阻塞地在子线程启动下载任务, 返回任务句柄.
    ///
    /// panic: 下载池被调用 cancel.
    pub fn download(&mut self, url: &str) -> DownloadHandle {
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
    fn join(mut self) -> Self::Result {
        self.handle.take().unwrap().join().unwrap(); // 下载池不应崩溃
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap(); // 下载池不应崩溃
        }
    }

    /// 询问下载任务是否均已完成
    fn is_finished(&self) -> bool {
        self.handle
            .as_ref()
            .is_none_or(|handle| handle.is_finished())
    }
}

impl_drop_for_handle! {DownloadPool}
