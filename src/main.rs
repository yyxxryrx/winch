mod find;
mod load;
mod lock_config;
mod msvc;
mod msys2;
mod project_config;
mod user_config;
mod util;
mod version;
mod winapi;

use crate::find::Tool;
use clap::{Parser, Subcommand};
use colored::Colorize;
use log::{debug, error, info};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

#[cfg(windows)]
const SPLITTER: char = ';';

#[derive(Debug, Parser)]
#[command(trailing_var_arg = true)]
struct RunArgs {
    program: String,
    #[arg(required = false)]
    args: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum SubCommand {
    /// Check the env
    Check,
    /// list all env
    List,
    /// into shell
    Shell,
    /// run program
    Run(RunArgs),
}

#[derive(Debug, Parser)]
struct Cli {
    #[arg(short, long)]
    verbose: bool,
    #[arg(short, long)]
    force: bool,
    #[clap(subcommand)]
    subcommand: SubCommand,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_config = user_config::load_config()?.unwrap_or_default();
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(if cli.verbose {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .parse_default_env()
        .init();
    match cli.subcommand {
        SubCommand::List => {
            let list: &[(Box<dyn Fn() -> version::GetVersionResult<Vec<Tool>>>, _)] = &[
                (Box::new(find::find_clang_with_version), "clang"),
                (Box::new(find::find_gcc_with_version), "gcc"),
                (Box::new(find::find_jdk_with_version), "java"),
            ];
            for (func, name) in list {
                println!("{name}:");
                match func() {
                    Ok(tools) => {
                        for tool in tools {
                            println!("{}: {}", tool.version, tool.path.display());
                        }
                    }
                    Err(e) => println!("error: {}", e),
                }
                println!();
            }
            if let Some(msys2_path) = msys2::find_msys2() {
                println!("msys2 found: {}", msys2_path.display());
            } else {
                println!("msys2 not found");
            }
            println!();
            let Some(vswhere) = msvc::try_get_vswhere() else {
                println!("Cannot find vswhere.exe");
                return Ok(());
            };
            let _ = msvc::get_all_vs(&vswhere).inspect(|v| println!("MSVC: {v:#?}"));
        }
        SubCommand::Shell | SubCommand::Run(..) | SubCommand::Check => {
            let envs = load(cli.force)?;
            match cli.subcommand {
                SubCommand::Shell => {
                    let shell = user_config.shell;
                    debug!("Running shell ... {}", shell.program);
                    std::process::Command::new(shell.program)
                        .args(shell.args)
                        .envs(envs)
                        .spawn()?
                        .wait()?;
                }
                SubCommand::Run(args) => {
                    debug!("Will run {} {}", &args.program, &args.args.join(" "));
                    let s = std::process::Command::new(&args.program)
                        .args(&args.args)
                        .envs(envs)
                        .spawn()?
                        .wait()?;
                    if s.success() {
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
                                s.code().unwrap_or_default()
                            )
                            .bright_red()
                        )
                    }
                }
                SubCommand::Check => {
                    println!("{}", "Check successfully completed".bright_green());
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}

fn load(force: bool) -> Result<HashMap<String, String>, toml::de::Error> {
    let config_file = Path::new("winch.toml");
    let lock_file = Path::new(".winch/cache.toml");
    if !config_file.exists() {
        error!("No config file found");
        std::process::exit(1);
    }
    let hash = util::sha256_file(config_file);
    let mut envs = std::env::vars().collect::<HashMap<_, _>>();

    if !force
        && lock_file.exists()
        && let Ok(lock) = std::fs::read_to_string(lock_file)
        && let Ok(lock) = toml::from_str::<lock_config::LockFile>(&lock)
    {
        debug!("Load from cache file");
        if hash.as_ref().is_ok_and(|s| &lock.hash == s) {
            for env in lock.envs {
                util::env_add(&mut envs, &env.name, &env.value)
            }
            info!("Load env from cache file successfully");
            return Ok(envs);
        }
        debug!("Hash not matched, will reload env");
    }

    match std::fs::read_to_string(config_file) {
        Ok(config) => {
            let config = toml::from_str::<project_config::Config>(&config)?;
            load_config(&config, &mut envs, hash.unwrap_or_default());
        }
        Err(e) => {
            error!("Error reading config file: {e}");
            std::process::exit(1);
        }
    }
    Ok(envs)
}

fn load_config(config: &project_config::Config, envs: &mut HashMap<String, String>, hash: String) {
    let dev_envs = match load::load_config(config) {
        Ok(e) => e,
        Err(e) => {
            match e {
                load::error::LoadError::CannotFind(things) => error!("Cannot find {things}"),
                load::error::LoadError::CannotFindTool(tool) => match tool {
                    load::error::ToolType::VSWhere => {
                        error!("Cannot find vswhere.exe");
                        println!(
                            "{}",
                            "Cannot find vswhere, you can use winget or scoop to install it"
                                .bright_red()
                        );
                        println!("{}", " - WinGet: winget install --id Microsoft.VisualStudio.Locator --source winget".bright_red());
                        println!("{}", " - Scoop: scoop install vswhere".bright_red());
                    }
                },
                load::error::LoadError::ConvertError(parse_error) => {
                    error!("Cannot parse version {parse_error}")
                }
                load::error::LoadError::GetEnvError(env_error) => {
                    error!("Cannot get env {env_error}")
                }
                load::error::LoadError::RunError(err) => error!("Run error: {err}"),
                load::error::LoadError::GetFailed(things) => error!("Cannot get {things}"),
                load::error::LoadError::GetVersionError(get_version_error) => {
                    error!("Get version failed: {get_version_error}")
                }
                load::error::LoadError::ParseVersionReqError(parse_error) => {
                    error!("Parse version requirements failed: {parse_error}")
                }
            }
            std::process::exit(1);
        }
    };
    util::env_merge(envs, &dev_envs);
    let envs = dev_envs
        .into_iter()
        .map(|(k, v)| lock_config::Env { name: k, value: v })
        .collect::<Vec<_>>();
    let lock = lock_config::LockFile { hash, envs };
    debug!("try to save cache file");
    if !std::fs::exists(".winch").is_ok_and(|b| b)
        && let Err(e) = std::fs::create_dir(".winch")
    {
        error!("Cannot create .winch directory ... {e}");
    }
    match toml::to_string(&lock) {
        Ok(lock) => match std::fs::File::create(".winch/cache.toml.tmp") {
            Ok(mut file) => {
                if let Err(e) = file.write_all(lock.as_bytes()) {
                    error!("Failed to write to cache.toml: {e}");
                } else if let Err(e) = std::fs::rename(".winch/cache.toml.tmp", ".winch/cache.toml")
                {
                    error!("Failed to write to cache.toml: {e}");
                }
            }
            Err(e) => error!("Cannot open cache.toml file: {e}"),
        },
        Err(e) => {
            error!("Failed to serialize cache file:\n{e}");
        }
    };
}
