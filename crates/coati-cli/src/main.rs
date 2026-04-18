use clap::{Parser, Subcommand};

mod cmd_ask;
mod cmd_hw;
mod cmd_model;
mod cmd_serve;
mod cmd_setup;
mod ipc;

#[derive(Parser)]
#[command(name = "coati", version, about = "Your Linux copilot.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ask a one-shot question and print the answer.
    Ask {
        /// The question. If omitted, reads from stdin.
        question: Option<String>,
    },
    /// Run as a daemon exposing a Unix socket.
    Serve {
        #[arg(long, default_value = "~/.cache/coati/agent.sock")]
        socket: String,
    },
    /// Print detected hardware and model recommendations.
    Hw,
    /// Manage LLM models.
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
    /// First-run TUI: detect hardware, pick a model, pull it, write config.
    Setup {
        /// Reset existing config and start over.
        #[arg(long)]
        reconfigure: bool,
        /// Skip prompts; pick the top recommended model.
        #[arg(long)]
        yes: bool,
        /// Override the model choice entirely (skips the picker).
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand)]
enum ModelAction {
    /// List models installed in ollama
    List,
    /// Pull a model via ollama
    Pull { name: String },
    /// Set the active model in config
    Set { name: String },
    /// Show hardware-based recommendations
    Recommend,
    /// Benchmark the currently-configured model
    Benchmark,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("coati=info")),
        )
        .init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Ask { question } => cmd_ask::run(question).await,
        Commands::Hw => cmd_hw::run().await,
        Commands::Serve { socket } => cmd_serve::run(&socket).await,
        Commands::Model { action } => match action {
            ModelAction::List => cmd_model::list().await,
            ModelAction::Pull { name } => cmd_model::pull(&name).await,
            ModelAction::Set { name } => cmd_model::set(&name),
            ModelAction::Recommend => cmd_model::recommend_cmd().await,
            ModelAction::Benchmark => cmd_model::benchmark().await,
        },
        Commands::Setup { reconfigure, yes, model } =>
            cmd_setup::run(reconfigure, yes, model).await,
    }
}
