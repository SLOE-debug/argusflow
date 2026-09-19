//! 独立激活对比 demo；不接入 workflow，不改生产激活策略。
mod fixture;
mod model;
mod runner;
mod strategies;
mod worker;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode, kind, ready] if mode == "fixture" => fixture::run(kind, ready),
        [mode, strategy, target, cover, output] if mode == "worker" => {
            worker::run(strategy.parse()?, target.parse()?, cover.parse()?, output).await
        }
        [mode, output, rounds] if mode == "compare" => {
            runner::run(output, rounds.parse()?, false).await
        }
        [mode, output, rounds, real] if mode == "compare" && real == "--real" => {
            runner::run(output, rounds.parse()?, true).await
        }
        _ => Err("用法：activation_compare compare <新输出目录> <轮数> [--real]".into()),
    }
}
