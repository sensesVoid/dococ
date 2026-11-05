use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dococ", version, about = "Doc Oc paper-first trading daemon CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show daemon status
    Status,
    /// Show current positions
    Positions,
    /// Show risk state
    Risk,
    /// Show version
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status) => {
            println!("Doc Oc daemon: not running (no daemon connected)");
        }
        Some(Commands::Positions) => {
            println!("No positions (daemon not connected)");
        }
        Some(Commands::Risk) => {
            println!("Risk state: unknown (daemon not connected)");
        }
        Some(Commands::Version) | None => {
            println!("dococ 0.1.0 (Phase 0)");
        }
    }
}
