use anyhow::Result;
use clap::{Parser, Subcommand};
use talaint_core::{ChangeFrequency, DataFormat, HttpSourceConfig, Purpose};

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
    /// Configure an HTTP data source (skeleton: parses and prints the config).
    Configure {
        /// URL of the data source.
        #[arg(long)]
        url: String,
        /// Format of the data.
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new([
            "csv", "json", "jsonl", "parquet", "xml",
        ]))]
        format: String,
        /// How often the data changes.
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new([
            "once", "hourly", "daily", "weekly", "monthly",
        ]))]
        change_frequency: String,
        /// Intended use of the data.
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new([
            "analytics", "rag", "agent", "search", "other",
        ]))]
        purpose: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Version => {
            println!("talaint {}", talaint_core::version());
        }
        Command::Configure {
            url,
            format,
            change_frequency,
            purpose,
        } => {
            let config = build_config(url, &format, &change_frequency, &purpose)?;
            print_config(&config);

            // TODO(Phase 2): persist to Postgres and attach a tenant.
            // TODO(Phase 3): fetch the URL, detect the real format, compare it
            //                with the declared format, and store the observation.
        }
    }

    Ok(())
}

/// Turn raw CLI strings into a validated config.
fn build_config(
    url: String,
    format: &str,
    change_frequency: &str,
    purpose: &str,
) -> Result<HttpSourceConfig> {
    let format: DataFormat = format.parse()?;
    let change_frequency: ChangeFrequency = change_frequency.parse()?;
    let purpose: Purpose = purpose.parse()?;

    Ok(HttpSourceConfig::new(
        url,
        format,
        change_frequency,
        purpose,
    ))
}

fn print_config(config: &HttpSourceConfig) {
    println!("configured HTTP source");
    println!("  url              : {}", config.url);
    println!("  format           : {}", config.format.as_str());
    println!("  change frequency : {}", config.change_frequency.as_str());
    println!("  purpose          : {}", config.purpose.as_str());
}
