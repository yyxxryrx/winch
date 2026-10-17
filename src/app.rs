use crate::cache::{self, CacheFile};
use crate::cli::{Cli, RunArgs, SubCommand};
use crate::find::{self, Tool};
use crate::msvc;
use crate::msys2;
use crate::toolchain::{LoadError, ToolType};
use crate::user_config;
use crate::version;
use colored::Colorize;
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("No winch.toml found in the current directory")]
    NoConfig,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    Toolchain(#[from] LoadError),
}

pub fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        SubCommand::List => {
            list();
            Ok(())
        }
        SubCommand::Shell => {
            let envs = load_environment(cli.force)?;
            shell(&envs)
        }
        SubCommand::Run(args) => {
            let envs = load_environment(cli.force)?;
            run_program(&args, &envs)
        }
        SubCommand::Check => {
            load_environment(cli.force)?;
            println!("{}", "Check successfully completed".bright_green());
            Ok(())
        }
    }
}

pub fn report(err: &AppError) {
    error!("{err}");
    if let AppError::Toolchain(LoadError::CannotFindTool(ToolType::VSWhere)) = err {
        println!(
            "{}",
            "Cannot find vswhere, you can use winget or scoop to install it".bright_red()
        );
        println!(
            "{}",
            " - WinGet: winget install --id Microsoft.VisualStudio.Locator --source winget"
                .bright_red()
        );
        println!("{}", " - Scoop: scoop install vswhere".bright_red());
    }
}

fn load_environment(force: bool) -> Result<HashMap<String, String>, AppError> {
    let config_path = Path::new("winch.toml");
    if !config_path.exists() {
        return Err(AppError::NoConfig);
    }
    let hash = cache::hash_file(config_path)?;
    let mut envs = std::env::vars().collect::<HashMap<_, _>>();

    if !force && let Some(cached) = cache::read(Path::new(cache::CACHE_PATH)) {
        if cached.matches(&hash) {
            debug!("Load from cache file");
            cached.env.apply_to(&mut envs);
            info!("Load env from cache file successfully");
            return Ok(envs);
        }
        debug!("Cache not matched, will reload env");
    }

    let config_text = std::fs::read_to_string(config_path)?;
    let config = toml::from_str::<crate::project_config::Config>(&config_text)?;
    let delta = crate::toolchain::resolve(&config)?;
    delta.apply_to(&mut envs);

    let cached = CacheFile {
        version: cache::FORMAT_VERSION,
        hash,
        env: delta,
    };
    if let Err(e) = cache::write(Path::new(cache::CACHE_PATH), &cached) {
        error!("Failed to save cache file: {e}");
    }

    Ok(envs)
}

fn list() {
    type VersionFinder = fn() -> version::GetVersionResult<Vec<Tool>>;

    let finders: [(&str, VersionFinder); 3] = [
        ("clang", find::find_clang_with_version),
        ("gcc", find::find_gcc_with_version),
        ("java", find::find_jdk_with_version),
    ];

    for (name, finder) in finders {
        println!("{name}:");
        match finder() {
            Ok(tools) => {
                for tool in tools {
                    println!("{}: {}", tool.version, tool.path.display());
                }
            }
            Err(e) => println!("error: {e}"),
        }
        println!();
    }

    match msys2::find_msys2() {
        Some(path) => println!("msys2 found: {}", path.display()),
        None => println!("msys2 not found"),
    }
    println!();

    let Some(vswhere) = msvc::try_get_vswhere() else {
        println!("Cannot find vswhere.exe");
        return;
    };
    let _ = msvc::get_all_vs(&vswhere).inspect(|v| println!("MSVC: {v:#?}"));
}

fn shell(envs: &HashMap<String, String>) -> Result<(), AppError> {
    let config = user_config::load_config()?.unwrap_or_default();
    debug!("Running shell ... {}", config.shell.program);
    Command::new(config.shell.program)
        .args(config.shell.args)
        .envs(envs)
        .spawn()?
        .wait()?;
    Ok(())
}

fn run_program(args: &RunArgs, envs: &HashMap<String, String>) -> Result<(), AppError> {
    debug!("Will run {} {}", args.program, args.args.join(" "));
    let status = Command::new(&args.program)
        .args(&args.args)
        .envs(envs)
        .spawn()?
        .wait()?;

    if status.success() {
        println!(
            "{}",
            format!("Run {} successfully", args.program).bright_green()
        );
    } else {
        eprintln!(
            "{}",
            format!(
                "Run {} exit with code {}",
                args.program,
                status.code().unwrap_or_default()
            )
            .bright_red()
        );
    }
    Ok(())
}
