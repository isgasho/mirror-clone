mod opam;
mod utils;
mod tar;

use std::path::PathBuf;
use async_log::span;

fn setup_logger() {
    let logger = femme::pretty::Logger::new();
    async_log::Logger::wrap(logger, || 12)
        .start(log::LevelFilter::Info)
        .unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let task = opam::Opam {
        base_path: PathBuf::from("data/opam"),
        repo: "http://localhost".to_string(),
        archive_url: "https://mirrors.sjtug.sjtu.edu.cn/opam-cache".to_string()
    };
    span!("opam", {
        task.run().await?;
    });
    Ok(())
}
