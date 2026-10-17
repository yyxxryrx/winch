use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub verbose: bool,
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
    /// Enter shell
    Shell,
    /// Run program
    Run(RunArgs),
}

#[derive(Debug, Parser)]
#[command(trailing_var_arg = true)]
pub struct RunArgs {
    pub program: String,
    #[arg(required = false)]
    pub args: Vec<String>,
}
