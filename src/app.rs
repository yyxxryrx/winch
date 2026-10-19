use crate::cache::{self, CacheFile};
use crate::cli::{Cli, RunArgs, SubCommand};
use crate::env::{EnvDelta, EnvObj};
use crate::find::{self, Tool};
use crate::msvc;
use crate::msys2;
use crate::project_config::Config;
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
    #[error("Environment check failed:\n  {}", .0.join("\n  "))]
    CheckFailed(Vec<String>),
    #[error(transparent)]
    Toolchain(#[from] LoadError),
    #[error(transparent)]
    Presets(#[from] crate::presets::cmake::GenerateError),
    #[error("JSONError: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        SubCommand::List => {
            list();
            Ok(())
        }
        SubCommand::Shell => {
            let env = load_environment(cli.force)?;
            shell(&env.vars, env.pure)
        }
        SubCommand::Run(args) => {
            let env = load_environment(cli.force)?;
            run_program(&args, &env.vars, env.pure)
        }
        SubCommand::Diff => {
            let env = load_environment(cli.force)?;
            crate::diff::print(&env.delta, &env.base, &env.vars);
            Ok(())
        }
        SubCommand::Check => {
            let config = read_config()?;
            let mut errors = crate::check::config::config_check(&config);

            if errors.is_empty() {
                match load_pure_environment_with(&config, cli.force) {
                    Ok(delta) => {
                        if let Err(e) = crate::check::env_delta::check_delta(&delta) {
                            errors.push(e.to_string());
                        }
                    }
                    Err(e) => errors.push(e.to_string()),
                }
            }

            if errors.is_empty() {
                println!("{}", "Check successfully completed".bright_green());
                Ok(())
            } else {
                Err(AppError::CheckFailed(errors))
            }
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
        SubCommand::Env => {
            let env = load_environment(cli.force)?;
            let mut entries = env.vars.into_iter().collect::<Vec<_>>();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in entries {
                println!("{key}={value}");
            }
            Ok(())
        }
        SubCommand::Preset(args) => {
            let config = read_config()?;
            let delta = load_pure_environment_with(&config, cli.force)?;
            if delta.pure {
                log::warn!(
                    "pure mode cannot clear inherited variables in CMake presets; only resolved variables are injected"
                );
            }
            match args.target {
                crate::cli::PresetTarget::Cmake(args) => {
                    crate::presets::cmake::generate(&delta, &config, &args.output)?;
                    info!("Saved presets to {}", args.output.display());
                }
            }
            Ok(())
        }
        SubCommand::Schema(args) => {
            let schema = crate::schema::generate()?;
            match args.output {
                Some(path) => {
                    if let Some(parent) = path
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                    {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, schema)?;
                    info!("Saved schema to {}", path.display());
                }
                None => print!("{schema}"),
            }
            Ok(())
        }
    }
}

pub fn report(err: &AppError) {
    match err {
        AppError::CheckFailed(_) => eprintln!("{}", err.to_string().bright_red()),
        _ => error!("{err}"),
    }
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

struct LoadedEnvironment {
    vars: HashMap<String, String>,
    base: HashMap<String, String>,
    delta: EnvDelta,
    pure: bool,
}

fn load_environment(force: bool) -> Result<LoadedEnvironment, AppError> {
    let delta = load_pure_environment(force)?;
    let pure = delta.pure;
    let base = std::env::vars().collect::<HashMap<_, _>>();
    let mut vars = base.clone();
    delta.apply_to(&mut vars);
    Ok(LoadedEnvironment {
        vars,
        base,
        delta,
        pure,
    })
}

fn read_config() -> Result<Config, AppError> {
    let config_path = Path::new("winch.toml");
    if !config_path.exists() {
        return Err(AppError::NoConfig);
    }
    let config_text = std::fs::read_to_string(config_path)?;
    Ok(toml::from_str::<Config>(&config_text)?)
}

fn load_pure_environment(force: bool) -> Result<crate::env::EnvDelta, AppError> {
    let config = read_config()?;
    load_pure_environment_with(&config, force)
}

fn load_pure_environment_with(
    config: &Config,
    force: bool,
) -> Result<crate::env::EnvDelta, AppError> {
    let config_path = Path::new("winch.toml");
    let hash = cache::hash_file(config_path)?;
    let cached = cache::read(Path::new(cache::CACHE_PATH));
    let hint = cached
        .as_ref()
        .and_then(|cache| cache.msys2_root.as_deref());
    let plan = crate::toolchain::plan(config, hint)?;
    let fingerprint = plan.fingerprint();

    if !force && let Some(cached) = cached {
        if cached.matches(&hash, &fingerprint, cache::now()) {
            debug!("Load from cache file");
            info!("Load env from cache file successfully");
            return Ok(cached.env);
        }
        debug!("Cache not matched, will reload env");
    }

    let mut delta = crate::toolchain::apply(&plan)?;

    delta.pure = config.pure;
    delta.keep = config
        .pure_keep
        .clone()
        .unwrap_or_else(crate::env::default_pure_keep);

    for (k, v) in &config.env.set {
        delta.set(k, EnvObj::new(v, "user config"));
    }
    for (k, v) in &config.env.prepend {
        delta.prepend(k, EnvObj::new(v, "user config"));
    }
    for (k, v) in &config.env.append {
        delta.append(k, EnvObj::new(v, "user config"));
    }

    let cached = CacheFile {
        version: cache::FORMAT_VERSION,
        hash,
        fingerprint,
        created_at: cache::now(),
        msys2_root: plan.msys2.as_ref().map(|msys2| msys2.root.clone()),
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

fn shell(envs: &HashMap<String, String>, pure: bool) -> Result<(), AppError> {
    let config = user_config::load_config()?.unwrap_or_default();
    debug!("Running shell ... {}", config.shell.program);
    let mut command = Command::new(config.shell.program);
    if pure {
        command.env_clear();
    }
    command.args(config.shell.args).envs(envs).spawn()?.wait()?;
    Ok(())
}

fn run_program(args: &RunArgs, envs: &HashMap<String, String>, pure: bool) -> Result<(), AppError> {
    debug!("Will run {} {}", args.program, args.args.join(" "));
    let mut command = Command::new(&args.program);
    if pure {
        command.env_clear();
    }
    let status = command.args(&args.args).envs(envs).spawn()?.wait()?;

    if status.success() {
        eprintln!(
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
