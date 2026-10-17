mod app;
mod cache;
mod check;
mod cli;
mod env;
mod export;
mod find;
mod msvc;
mod msys2;
mod project_config;
mod toolchain;
mod user_config;
mod version;
mod winapi;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    env_logger::Builder::new()
        .filter_level(if cli.verbose {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .parse_default_env()
        .init();

    match app::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            app::report(&e);
            ExitCode::FAILURE
        }
    }
}
