//! bd2wg 命令行终端

mod utils;

use std::{thread::sleep, time::Duration};

use bd2wg::{
    services::pipeline::TranspilePipeline,
    traits::{
        handle::Handle,
        pipeline::{DownloadResult, DownloadState, TranspileResult, TranspileState},
    },
    utils::*,
};
use indicatif::{ProgressBar, ProgressStyle};

use crate::utils::*;

const GIT_REPOSITORY: &str = "https://github.com/fltLi/bd2wg";

/// 状态更新间隔
const STATE_UPDATE_BACKOFF: Duration = Duration::from_millis(100);

/// 单次工作
fn run() {
    println!();

    let story = readln! {"script"};
    let outdir = readln! {"outdir"};

    // 转译

    println!("transpiling...");
    flush! {};

    let pipe = TranspilePipeline::new(story, outdir, default_header().unwrap());

    let (
        TranspileResult {
            state: TranspileState { scene, action },
            errors,
        },
        pipe,
    ) = pipe.join(); // 转译很快, 直接阻塞等待即可.

    println!("translation completed, result: ");
    print!("{scene} scenes, {action} actions, ");
    try_show_errors(errors);

    println!();
    flush! {};

    // 下载

    let pipe = match pipe {
        Ok(v) => v,
        Err(e) => {
            println!("failed to start download, error:\n{e}");
            flush! {};
            return;
        }
    };

    println!("downloading...");
    flush! {};

    // 初始化 indicatif 进度条
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    // 等待下载完成
    while !pipe.is_finished() {
        let DownloadState { done, total } = pipe.state();

        // 使用进度条呈现 done / total
        pb.set_length(total as u64);
        pb.set_position(done as u64);

        sleep(STATE_UPDATE_BACKOFF);
    }

    let DownloadResult {
        state: DownloadState { total, done },
        errors,
    } = pipe.join();

    pb.set_length(total as u64);
    pb.set_position(done as u64);
    pb.finish();

    println!("download completed, result: ");
    print!("{} success, ", total - errors.len());
    try_show_errors(errors);

    pause! {};
}

fn main() {
    println!("bd2wg-cli\n{GIT_REPOSITORY}");
    flush! {};

    loop {
        run();
    }
}
