use anyhow::Result;
use clap::{Parser, Subcommand};

/// TALAINT: a tenant-aware AI data layer.
#[derive(Debug, Parser)]
#[command(name = "talaint", version, about = "A tenant-aware AI data layer")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print build information.
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Version => {
            println!("talaint {}", talaint_core::version());
        }
    }

    Ok(())
}
