use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// A declarative environment manager
#[derive(Debug, Parser)]
pub struct Cli {
    /// Disable log output
    #[clap(long)]
    pub no_log: bool,
    /// Show debug log
    #[arg(short, long)]
    pub verbose: bool,
    /// Do not use cache
    #[arg(short, long)]
    pub force: bool,
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
}

#[derive(Debug, Parser)]
#[command(trailing_var_arg = true)]
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
