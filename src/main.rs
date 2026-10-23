mod app;
mod cache;
mod check;
mod cli;
mod diff;
mod env;
mod export;
mod find;
mod msvc;
mod msys2;
mod output;
mod presets;
mod project_config;
mod schema;
mod toolchain;
mod user_config;
mod version;
mod winapi;

use clap::Parser;
use std::io::Write;

fn main() {
    let cli = cli::Cli::parse();
    output::init(cli.verbose, cli.no_log);

    let code = match app::run(cli) {
        Ok(code) => code,
        Err(e) => {
            app::report(&e);
            1
        }
    };
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}
