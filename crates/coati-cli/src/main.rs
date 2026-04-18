use clap::{Parser, Subcommand};

mod cmd_ask;
mod cmd_explain;
mod cmd_hw;
mod cmd_model;
mod cmd_propose;
mod cmd_serve;
mod cmd_setup;
mod ipc;
#[cfg(feature = "voice")]
mod cmd_voice;

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
    /// Propose a shell command for a natural-language intent.
    Propose {
        /// The intent, e.g. "restart nginx"
        intent: String,
        /// Emit machine-readable JSON instead of human text.
        #[arg(long)]
        json: bool,
        /// Pre-captured shell context as JSON (overrides auto-detection).
        #[arg(long)]
        context: Option<String>,
    },
    /// Explain why a command failed, with an optional fix.
    Explain {
        #[arg(long)]
        command: String,
        #[arg(long, default_value = "")]
        stdout: String,
        #[arg(long, default_value = "")]
        stderr: String,
        #[arg(long)]
        exit: i32,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        context: Option<String>,
    },
    /// Voice commands (requires --features voice at build time).
    #[cfg(feature = "voice")]
    Voice {
        #[command(subcommand)]
        action: VoiceAction,
    },
}

#[cfg(feature = "voice")]
#[derive(Subcommand)]
enum VoiceAction {
    /// Download and install a whisper model.
    Setup {
        #[arg(long, default_value = "base.en")]
        model: String,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Transcribe a 16kHz mono WAV file and print the text.
    Transcribe {
        path: std::path::PathBuf,
        #[arg(long, default_value = "base.en")]
        model: String,
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
        Commands::Setup {
            reconfigure,
            yes,
            model,
        } => cmd_setup::run(reconfigure, yes, model).await,
        Commands::Propose {
            intent,
            json,
            context,
        } => cmd_propose::run(&intent, json, context.as_deref()).await,
        Commands::Explain {
            command,
            stdout,
            stderr,
            exit,
            json,
            context,
        } => cmd_explain::run(&command, &stdout, &stderr, exit, json, context.as_deref()).await,
        #[cfg(feature = "voice")]
        Commands::Voice { action } => match action {
            VoiceAction::Setup { model, yes } => cmd_voice::setup(&model, yes).await,
            VoiceAction::Transcribe { path, model } => cmd_voice::transcribe_file(&path, &model).await,
        },
    }
}
