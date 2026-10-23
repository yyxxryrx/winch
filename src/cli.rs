use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// A declarative environment manager
#[derive(Debug, Parser)]
pub struct Cli {
    /// Disable status output
    #[arg(short = 'q', long = "quiet", alias = "no-log")]
    pub no_log: bool,
    /// Show debug output
    #[arg(short, long)]
    pub verbose: bool,
    /// Do not use cache
    #[arg(short, long)]
    pub force: bool,
    /// Path to the project configuration
    #[arg(short, long, default_value = "winch.toml")]
    pub config: PathBuf,
    #[clap(subcommand)]
    pub command: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    /// Check the env
    Check,
    /// Show environment changes
    Diff,
    /// Export env
    Export(ExportArgs),
    /// Print env
    Env,
    /// List all env
    List,
    /// Enter shell with env
    Shell,
    /// Run program with env
    Run(RunArgs),
    /// Generate presets
    Preset(PresetArgs),
    /// Generate configuration schema
    Schema(SchemaArgs),
}

#[derive(Debug, Parser)]
pub struct SchemaArgs {
    /// Output file (stdout by default)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct PresetArgs {
    #[clap(subcommand)]
    pub target: PresetTarget,
}

#[derive(Debug, Subcommand)]
pub enum PresetTarget {
    /// Generate CMake presets
    Cmake(CmakePresetArgs),
}

#[derive(Debug, Parser)]
pub struct CmakePresetArgs {
    /// Output file
    #[arg(short, long, default_value = "CMakeUserPresets.json")]
    pub output: PathBuf,
}

#[derive(Debug, Parser)]
#[command(trailing_var_arg = true, allow_hyphen_values = true)]
pub struct RunArgs {
    pub program: String,
    #[arg(required = false)]
    pub args: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct ExportArgs {
    /// The shell to export the script
    #[clap(long)]
    pub shell: SupportedShell,
    #[clap(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SupportedShell {
    Bash,
    #[clap(name = "powershell")]
    PowerShell,
    Nu,
}

impl SupportedShell {
    pub fn extension(&self) -> &'static str {
        match self {
            SupportedShell::Bash => ".sh",
            SupportedShell::PowerShell => ".ps1",
            SupportedShell::Nu => ".nu",
        }
    }
}
