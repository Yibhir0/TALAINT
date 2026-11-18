mod cmd;
mod creds;
mod output;

use clap::{Args, Parser, Subcommand, ValueEnum};
use output::OutputMode;

#[derive(Parser, Debug)]
#[command(
    name = "talaint",
    version,
    about = "Talaint CLI - tenant-aware AI data ingestion & preparation"
)]
struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFmt::Text)]
    output: OutputFmt,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFmt {
    Text,
    Json,
}
impl From<OutputFmt> for OutputMode {
    fn from(v: OutputFmt) -> Self {
        match v {
            OutputFmt::Text => OutputMode::Text,
            OutputFmt::Json => OutputMode::Json,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Authenticate CLI
    Auth {
        #[command(subcommand)]
        cmd: AuthCmd,
    },
    /// Rotate current tenant token
    Token {
        #[command(subcommand)]
        cmd: TokenCmd,
    },
    /// Print config templates
    Init {
        #[arg(long, conflicts_with = "intent")]
        source: Option<String>,
        #[arg(long, conflicts_with = "source")]
        intent: Option<String>,
    },
    /// Generate a validated pipeline YAML file
    Generate {
        #[arg(long)]
        intent: String,
        #[arg(short = 'f', long = "file")]
        file: String,
        #[arg(long = "source")]
        sources: Vec<String>,
        #[arg(long, conflicts_with = "force")]
        append: bool,
        #[arg(long, conflicts_with = "append")]
        force: bool,
    },
    /// Discovery lists
    List {
        #[arg(value_parser = ["sources", "intents", "formats"])]
        kind: String,
    },
    /// Validate a pipeline YAML config
    Validate {
        #[arg(short = 'f', long = "file")]
        file: String,
    },
    /// Apply a pipeline YAML config
    Apply {
        #[arg(short = 'f', long = "file")]
        file: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
    },
    /// Source operations
    Source {
        #[command(subcommand)]
        cmd: SourceCmd,
    },
    /// Pipeline operations
    Pipelines {
        #[command(subcommand)]
        cmd: PipelinesCmd,
    },
    /// Runs and logs
    Runs {
        #[command(subcommand)]
        cmd: RunsCmd,
    },
    /// Secret operations
    Secrets {
        #[command(subcommand)]
        cmd: SecretsCmd,
    },
    /// Pause a source schedule
    Pause(SourceSelector),
    /// Resume a source schedule
    Resume(SourceSelector),
    /// Conversational configuration assistant
    Chat,
    /// Admin operations
    Admin {
        #[command(subcommand)]
        cmd: AdminCmd,
    },
}

#[derive(Subcommand, Debug)]
enum AuthCmd {
    Login {
        #[arg(long)]
        token: String,
        #[arg(long)]
        admin: bool,
    },
}
#[derive(Subcommand, Debug)]
enum TokenCmd {
    Rotate,
}

#[derive(Subcommand, Debug)]
enum SourceCmd {
    /// Test source connectivity/health from config file
    Test {
        #[arg(short = 'f', long = "file")]
        file: String,
    },
    /// Delete a source from a pipeline intent
    Delete {
        #[arg(long)]
        intent: String,
        #[arg(long = "source")]
        source: String,
        #[arg(long)]
        purge_data: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PipelinesCmd {
    List,
    Show {
        #[arg(long)]
        intent: String,
    },
    Delete {
        #[arg(long)]
        intent: String,
        #[arg(long)]
        purge_data: bool,
    },
}

#[derive(Subcommand, Debug)]
enum RunsCmd {
    List {
        #[arg(long)]
        intent: Option<String>,
        #[arg(long)]
        source: Option<String>,
    },
    Show {
        run_id: String,
    },
    Logs {
        run_id: String,
    },
    Trigger {
        #[arg(long)]
        intent: String,
        #[arg(long)]
        source: String,
    },
}

#[derive(Subcommand, Debug)]
enum SecretsCmd {
    Set { name: String },
    List,
    Delete { name: String },
}

#[derive(Subcommand, Debug)]
enum AdminCmd {
    #[command(subcommand)]
    Tenant(AdminTenantCmd),
    #[command(subcommand)]
    Token(AdminTokenCmd),
}
#[derive(Subcommand, Debug)]
enum AdminTenantCmd {
    Create {
        #[arg(long)]
        name: String,
    },
    List,
}
#[derive(Subcommand, Debug)]
enum AdminTokenCmd {
    Rotate {
        #[arg(long)]
        tenant: String,
    },
}

#[derive(Args, Debug)]
struct SourceSelector {
    #[arg(long)]
    intent: String,
    #[arg(long)]
    source: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mode: OutputMode = cli.output.into();
    let res = match cli.command {
        Commands::Auth { cmd } => match cmd {
            AuthCmd::Login { token, admin } => cmd::cmd_auth_login(mode, &token, admin).await,
        },
        Commands::Token { cmd } => match cmd {
            TokenCmd::Rotate => cmd::cmd_token_rotate(mode).await,
        },
        Commands::Init { source, intent } => {
            cmd::cmd_init(mode, source.as_deref(), intent.as_deref()).await
        }
        Commands::Generate {
            intent,
            file,
            sources,
            append,
            force,
        } => cmd::cmd_generate(mode, &intent, &file, &sources, append, force).await,
        Commands::List { kind } => cmd::cmd_list(mode, &kind).await,
        Commands::Validate { file } => cmd::cmd_validate(mode, &file).await,
        Commands::Apply {
            file,
            dry_run,
            force,
        } => cmd::cmd_apply(mode, &file, dry_run, force).await,
        Commands::Source { cmd } => match cmd {
            SourceCmd::Test { file } => cmd::cmd_source_test(mode, &file).await,
            SourceCmd::Delete {
                intent,
                source,
                purge_data,
            } => cmd::cmd_source_delete(mode, &intent, &source, purge_data).await,
        },
        Commands::Pipelines { cmd } => match cmd {
            PipelinesCmd::List => cmd::cmd_pipelines_list(mode).await,
            PipelinesCmd::Show { intent } => cmd::cmd_pipelines_show(mode, &intent).await,
            PipelinesCmd::Delete { intent, purge_data } => {
                cmd::cmd_pipelines_delete(mode, &intent, purge_data).await
            }
        },
        Commands::Runs { cmd } => match cmd {
            RunsCmd::List { intent, source } => {
                cmd::cmd_runs_list(mode, intent.as_deref(), source.as_deref()).await
            }
            RunsCmd::Show { run_id } => cmd::cmd_runs_show(mode, &run_id).await,
            RunsCmd::Logs { run_id } => cmd::cmd_runs_logs(mode, &run_id).await,
            RunsCmd::Trigger { intent, source } => {
                cmd::cmd_runs_trigger(mode, &intent, &source).await
            }
        },
        Commands::Secrets { cmd } => match cmd {
            SecretsCmd::Set { name } => cmd::cmd_secrets_set(mode, &name).await,
            SecretsCmd::List => cmd::cmd_secrets_list(mode).await,
            SecretsCmd::Delete { name } => cmd::cmd_secrets_delete(mode, &name).await,
        },
        Commands::Pause(sel) => cmd::cmd_pause_resume(mode, &sel.intent, &sel.source, true).await,
        Commands::Resume(sel) => cmd::cmd_pause_resume(mode, &sel.intent, &sel.source, false).await,
        Commands::Chat => {
            if mode.is_json() {
                let _ = output::print_json(
                    &serde_json::json!({"ok": false, "error": "chat is not implemented yet (M6)"}),
                );
                std::process::exit(1);
            } else {
                println!("`talaint chat` (conversational agent harness) is planned for M6 and not implemented yet.");
                Ok(())
            }
        }
        Commands::Admin { cmd } => match cmd {
            AdminCmd::Tenant(tc) => match tc {
                AdminTenantCmd::Create { name } => cmd::cmd_admin_tenant_create(mode, &name).await,
                AdminTenantCmd::List => cmd::cmd_admin_tenant_list(mode).await,
            },
            AdminCmd::Token(tk) => match tk {
                AdminTokenCmd::Rotate { tenant } => {
                    cmd::cmd_admin_token_rotate(mode, &tenant).await
                }
            },
        },
    };
    cmd::handle_result(mode, res);
}
