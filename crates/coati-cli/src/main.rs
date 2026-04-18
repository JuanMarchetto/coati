use clap::{Parser, Subcommand};

mod cmd_ask;
mod cmd_serve;

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
        Commands::Serve { socket } => cmd_serve::run(&socket).await,
    }
}
