mod auth;
mod client;
mod commands;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, ValueEnum)]
enum StartupFormat {
    /// Plain text to stdout (Claude Code).
    Text,
    /// JSON with `hookSpecificOutput.additionalContext` (Gemini CLI).
    Gemini,
}

#[derive(Parser)]
#[command(
    name = "bl",
    about = "Baselayer CLI — terminal interface to your knowledge vault"
)]
#[command(version, propagate_version = true)]
struct Cli {
    /// API base URL (default: https://api.baselayer.id)
    #[arg(long, env = "BASELAYER_API_URL")]
    api_url: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show compact session context (designed for IDE hooks)
    Startup {
        /// Output format: text (Claude Code) or gemini (Gemini CLI hook JSON).
        #[arg(long, value_enum, default_value_t = StartupFormat::Text)]
        format: StartupFormat,
    },

    /// Ask a question and get a synthesized answer from your vault
    Ask {
        /// The question to ask
        question: String,
    },

    /// Search your vault for entities and facts
    Search {
        /// Search query
        query: String,
    },

    /// Record a memory for future recall
    Remember {
        /// The text to remember
        text: String,

        /// Entity to attach the memory to
        #[arg(long)]
        attach_to: Option<String>,
    },

    /// Manage authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },

    /// Set up IDE integrations (hooks)
    Setup {
        #[command(subcommand)]
        command: SetupCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Sign in via browser OAuth
    Login,
    /// Show current auth status
    Status,
    /// Sign out and clear stored credentials
    Logout,
}

#[derive(Subcommand)]
enum SetupCommands {
    /// Install Claude Code session hooks
    Claude {
        /// Remove hooks instead of installing
        #[arg(long)]
        remove: bool,
    },
    /// Install Gemini CLI session hooks
    Gemini {
        /// Remove hooks instead of installing
        #[arg(long)]
        remove: bool,
    },
    /// Check installation status of all integrations
    Check,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let api_url = cli
        .api_url
        .unwrap_or_else(|| "https://api.baselayer.id".to_string());

    let result = match cli.command {
        Commands::Startup { format } => {
            let fmt = match format {
                StartupFormat::Text => commands::startup::Format::Text,
                StartupFormat::Gemini => commands::startup::Format::Gemini,
            };
            commands::startup::run(&api_url, fmt).await
        }
        Commands::Ask { question } => commands::ask::run(&api_url, &question).await,
        Commands::Search { query } => commands::search::run(&api_url, &query).await,
        Commands::Remember { text, attach_to } => {
            commands::remember::run(&api_url, &text, attach_to.as_deref()).await
        }
        Commands::Auth { command } => match command {
            AuthCommands::Login => commands::auth::login(&api_url).await,
            AuthCommands::Status => commands::auth::status(&api_url).await,
            AuthCommands::Logout => commands::auth::logout().await,
        },
        Commands::Setup { command } => match command {
            SetupCommands::Claude { remove } => commands::setup::claude(remove).await,
            SetupCommands::Gemini { remove } => commands::setup::gemini(remove).await,
            SetupCommands::Check => commands::setup::check().await,
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
