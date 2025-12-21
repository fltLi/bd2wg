//! 命令行辅助工具

use bd2wg::Error;

#[macro_export]
macro_rules! flush {
    () => {{
        use std::io::{Write, stdout};

        stdout().flush().unwrap()
    }};
}

/// 读取当前行
#[macro_export]
macro_rules! readln {
    () => {
        std::io::stdin()
            .lines()
            .next()
            .expect("输入已结束")
            .unwrap()
    };

    ($($arg:tt)+) => {{
        print!($($arg)+);
        print!(": ");
        flush! {};
        readln! {}
    }};
}

/// 等待
#[macro_export]
macro_rules! pause {
    () => {{
        let _ = readln! {"press any key to continue...\n"};
    }};
}

/// 展示错误
pub fn try_show_errors(errs: impl AsRef<[Error]>) {
    let errs = errs.as_ref();

    if errs.is_empty() {
        println!("no error.");
    } else {
        println!("{} errors: ", errs.len());

        for (k, err) in errs.iter().enumerate() {
            println!("{}. {}.", k + 1, err);
        }
    }

    flush!()
}
