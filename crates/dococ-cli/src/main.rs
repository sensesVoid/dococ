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
    /// Show account state (equity, drawdown, daily PnL)
    Acct,
    /// Enable/disable auto-trading
    Auto {
        /// on or off
        state: String,
    },
    /// Flatten all positions and halt
    Kill,
    /// Show current positions
    Positions,
    /// Show risk state
    Risk,
    /// Force position reconciliation with venue
    Reconcile,
    /// Run historical backtest
    Backtest {
        /// Path to data file
        data: String,
        /// Strategy to backtest (teacher, laya)
        #[arg(default_value = "teacher")]
        strategy: String,
    },
    /// Start paper trading mode
    Paper {
        /// Asset to paper trade
        #[arg(default_value = "BTCUSDT")]
        asset: String,
    },
    /// Show version
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status) => {
            println!("Doc Oc daemon: not running (no daemon connected)");
        }
        Some(Commands::Acct) => {
            println!("Account: not connected to daemon");
            println!("  Equity:     N/A");
            println!("  Daily PnL:  N/A");
            println!("  Drawdown:   N/A");
            println!("  Peak:       N/A");
        }
        Some(Commands::Auto { state }) => {
            match state.as_str() {
                "on" => println!("Auto-trading: ENABLED (daemon not connected)"),
                "off" => println!("Auto-trading: DISABLED (daemon not connected)"),
                _ => println!("Usage: dococ auto on|off"),
            }
        }
        Some(Commands::Kill) => {
            println!("KILL: Flattening all positions and halting (daemon not connected)");
        }
        Some(Commands::Positions) => {
            println!("No positions (daemon not connected)");
        }
        Some(Commands::Risk) => {
            println!("Risk state: unknown (daemon not connected)");
        }
        Some(Commands::Reconcile) => {
            println!("Reconciling positions with venue (daemon not connected)");
        }
        Some(Commands::Backtest { data, strategy }) => {
            println!("Backtest: strategy={}, data={}", strategy, data);
            println!("  (not yet implemented)");
        }
        Some(Commands::Paper { asset }) => {
            println!("Paper trading: {} (daemon not connected)", asset);
        }
        Some(Commands::Version) | None => {
            println!("dococ 0.1.0 (Phase 1)");
        }
    }
}
