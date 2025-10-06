//! bd2wg 终端

use std::io::Write;
use std::path::PathBuf;
use std::{env, io, thread, time};

use bd2wg::error::Error;
use bd2wg::pipeline::{DefaultPipeline, Pipeline, Procedure, State};

const FLUSH_TIME: time::Duration = time::Duration::from_secs(5);
const GIT_REPOSITORY: &str = "https://github.com/fltLi/bd2wg";

/// 基本输入
macro_rules! input {
    () => {{
        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();
        line.trim().to_string()
    }};
    ($prompt:literal) => {{
        print!($prompt);
        flush!();
        input!()
    }};
}

macro_rules! flush {
    () => {
        io::stdout().flush().unwrap()
    };
}

struct Renderer {
    error: Vec<Error>,
}

impl Renderer {
    fn new() -> Self {
        Self { error: vec![] }
    }

    fn show(&mut self, state: State) {
        println!("procedure: {}\n", state.procedure);

        println!("scene count: {}", state.scene_count);
        println!("download task count: {}\n", state.download_task_count);

        println!("purified action count: {}", state.purified_action_count);
        println!(
            "transpiled action count: {}\n",
            state.transpiled_action_count
        );

        self.error.extend(state.error);
        println!("errors:");
        self.error
            .iter()
            .enumerate()
            .for_each(|(id, err)| println!("  {id}. {err}"));

        self.flush();
    }

    fn flush(&self) {
        println!("\n----------------\n");
        flush!();
    }
}

fn main() {
    println!("bd2wg-cli\n{GIT_REPOSITORY}\n");
    println!("current dir: {}\n", env::current_dir().unwrap().to_str().unwrap());

    let story: PathBuf = input!("story file: ").into();
    let mut project = input!("project root: ");
    if !project.ends_with('/') {
        project += "/";
    }

    let mut pipeline = DefaultPipeline::new(&story, project);
    pipeline.process().unwrap();

    let mut renderer = Renderer::new();

    let mut run = true;
    while run {
        let state = pipeline.take_state().unwrap();
        run = state.procedure != Procedure::Completed;

        renderer.show(state);
        thread::sleep(FLUSH_TIME);
    }

    input!("\nCompleted!\n");
}
