use clap::{Parser, Subcommand};

/// A declarative environment manager
#[derive(Debug, Parser)]
pub struct Cli {
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
