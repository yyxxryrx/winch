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
use std::io::Write;
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
            let delta = load_pure_environment(cli.force)?;
            if let Err(e) = crate::check::check_delta(&delta) {
                match e {
                    crate::check::error::CheckError::NotExists(p) => {
                        error!("Path was not exists ... {p}")
                    }
                }
            }
            println!("{}", "Check successfully completed".bright_green());
            Ok(())
        }
        SubCommand::Export(args) => {
            let delta = load_pure_environment(cli.force)?;
            let script = match args.shell {
                crate::cli::SupportedShell::Nu => crate::export::export_nu(&delta),
                crate::cli::SupportedShell::Bash => crate::export::export_bash(&delta),
                crate::cli::SupportedShell::PowerShell => crate::export::export_powershell(&delta),
            };
            let dir = Path::new(".winch");
            if !std::fs::exists(dir)? {
                std::fs::create_dir(dir)?;
            }
            let output = args
                .output
                .unwrap_or_else(|| dir.join(format!("env{}", args.shell.extension())));
            if output.as_os_str() == "-" {
                print!("{script}");
                return Ok(());
            }
            info!("Save to {}", output.display());
            let mut file = std::fs::File::create(output)?;
            file.write_all(script.as_bytes())?;
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
    let mut envs = std::env::vars().collect::<HashMap<_, _>>();
    load_pure_environment(force)?.apply_to(&mut envs);
    Ok(envs)
}

fn load_pure_environment(force: bool) -> Result<crate::env::EnvDelta, AppError> {
    let config_path = Path::new("winch.toml");
    if !config_path.exists() {
        return Err(AppError::NoConfig);
    }
    let hash = cache::hash_file(config_path)?;

    if !force && let Some(cached) = cache::read(Path::new(cache::CACHE_PATH)) {
        if cached.matches(&hash) {
            debug!("Load from cache file");
            info!("Load env from cache file successfully");
            return Ok(cached.env);
        }
        debug!("Cache not matched, will reload env");
    }

    let config_text = std::fs::read_to_string(config_path)?;
    let config = toml::from_str::<crate::project_config::Config>(&config_text)?;
    let mut delta = crate::toolchain::resolve(&config)?;

    for (k, v) in config.env.iter() {
        delta.set(k, v);
    }

    let cached = CacheFile {
        version: cache::FORMAT_VERSION,
        hash,
        env: delta.clone(),
    };
    if let Err(e) = cache::write(Path::new(cache::CACHE_PATH), &cached) {
        error!("Failed to save cache file: {e}");
    }

    Ok(delta)
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
