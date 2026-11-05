use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "dococ",
    version,
    about = "Doc Oc paper-first trading daemon CLI"
)]
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
    /// Start paper trading mode (simulated orders via SimVenue)
    Paper {
        /// Asset to paper trade
        #[arg(default_value = "BTCUSDT")]
        asset: String,
        /// Number of recent live bars to process
        #[arg(default_value_t = 10)]
        bars: usize,
    },
    /// Run shadow mode: log decisions without placing orders
    Shadow {
        /// Asset to shadow-trade
        #[arg(default_value = "BTCUSDT")]
        asset: String,
        /// Number of bars to replay
        #[arg(default_value_t = 10)]
        bars: usize,
    },
    /// Show version
    Version,
    /// Manage asset packs
    Asset {
        #[command(subcommand)]
        action: AssetAction,
    },
    /// Open the dashboard skeleton (ratatui)
    Dash {
        /// Asset symbol to show (reads paper_<asset>.json if present)
        #[arg(default_value = "BTCUSDT")]
        asset: String,
    },
}

#[derive(Subcommand)]
enum AssetAction {
    /// Add a new asset pack
    Add { asset: String },
    /// List asset packs
    List,
    /// Show an asset pack
    Show { asset: String },
    /// Promote an asset pack to the next stage
    Promote { asset: String },
    /// Demote an asset pack to the previous stage
    Demote { asset: String },
    /// Remove an asset pack
    Rm { asset: String },
}

fn assets_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("assets")
}

fn read_pack(asset: &str) -> Option<dococ_asset::AssetPack> {
    let path = assets_dir().join(format!("{}.json", asset.to_uppercase()));
    let text = std::fs::read_to_string(path).ok()?;
    dococ_asset::AssetPack::from_json(&text).ok()
}

fn write_pack(pack: &dococ_asset::AssetPack) {
    let _ = std::fs::create_dir_all(assets_dir());
    let path = assets_dir().join(format!("{}.json", pack.asset.to_uppercase()));
    std::fs::write(path, pack.to_json().unwrap()).unwrap();
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
        Some(Commands::Auto { state }) => match state.as_str() {
            "on" => println!("Auto-trading: ENABLED (daemon not connected)"),
            "off" => println!("Auto-trading: DISABLED (daemon not connected)"),
            _ => println!("Usage: dococ auto on|off"),
        },
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
        Some(Commands::Paper { asset, bars: n_bars }) => {
            let model_dir = std::env::var("DOCOC_LAYA_MODEL")
                .unwrap_or_else(|_| "models/btcusdt_ml".to_string());
            let symbol = asset.to_uppercase();
            println!(
                "Paper trading: {} (model: {}, bars: {})",
                symbol, model_dir, n_bars
            );
            match dococ_laya::LayaRuntime::load(&model_dir) {
                Ok(laya) => {
                    let strategy = dococ_engine::LayaStrategy::new(laya);
                    let venue = dococ_venue::SimVenue::new(100_000.0, 5.0);
                    let bars = dococ_venue::live::fetch_latest(&symbol, n_bars).unwrap_or_default();
                    if bars.is_empty() {
                        println!("No live bars fetched; cannot start paper loop.");
                        return;
                    }
                    let account = dococ_core::Account::new(dococ_core::AccountId(1), 100_000.0);
                    let scheduler = dococ_engine::Scheduler::new(900, 1);
                    let mut engine = dococ_engine::Engine::new(
                        strategy,
                        dococ_risk::RiskConfig::default(),
                        venue,
                        account,
                        scheduler,
                    );
                    let sym = dococ_core::Symbol::new(&symbol);
                    let mut journal: Vec<serde_json::Value> = Vec::new();
                    let mut ticks = 0usize;
                    for (ts, close) in &bars {
                        let bar = dococ_core::Bar {
                            timestamp: *ts,
                            open: *close,
                            high: *close,
                            low: *close,
                            close: *close,
                            volume: 0.0,
                        };
                        if let Ok(Some(tick)) = engine.on_bar(&bar, &sym) {
                            ticks += 1;
                            let pos_qty = engine
                                .account()
                                .get_position(&sym)
                                .map(|p| p.quantity)
                                .unwrap_or(0.0);
                            let risk = format!("{:?}", tick.risk_decision);
                            println!(
                                "  {} {:?} conf={:.3} target={:.4} pos={:.4} risk={}",
                                ts, tick.action, tick.action_confidence, tick.target_qty, pos_qty, risk
                            );
                            journal.push(serde_json::json!({
                                "timestamp": ts,
                                "symbol": symbol,
                                "price": close,
                                "action": format!("{:?}", tick.action),
                                "confidence": tick.action_confidence,
                                "target_qty": tick.target_qty,
                                "risk_decision": risk,
                                "position_qty": pos_qty,
                                "equity": engine.account().equity,
                                "drawdown": engine.account().drawdown(),
                                "oms_filled": engine.oms().filled_orders().len(),
                            }));
                        }
                    }
                    let acct = engine.account();
                    let out = std::path::PathBuf::from(format!("paper_{}.json", symbol.to_lowercase()));
                    let _ = std::fs::write(
                        &out,
                        serde_json::to_string_pretty(&journal).unwrap_or_else(|_| "[]".to_string()),
                    );
                    println!(
                        "Paper complete: {} tick(s), equity {:.2}, DD {:.2}%, filled orders {}, journal {}",
                        ticks,
                        acct.equity,
                        acct.drawdown() * 100.0,
                        engine.oms().filled_orders().len(),
                        out.display()
                    );
                }
                Err(e) => println!("Failed to load model: {e}"),
            }
        }
        Some(Commands::Shadow { asset, bars }) => {
            match std::env::var("DOCOC_LAYA_MODEL") {
                Ok(model_dir) => {
                    let csv = std::path::PathBuf::from("data/btcusdt_1h.csv");
                    match dococ_shadow::run_shadow(
                        std::path::Path::new(&model_dir),
                        &csv,
                        bars,
                        &asset.to_uppercase(),
                    ) {
                        Ok(log) => {
                            println!(
                                "Shadow mode: {} over {} decision(s), orders placed: {}",
                                asset, log.decisions, log.orders
                            );
                            let out = std::path::PathBuf::from(format!(
                                "shadow_{}.json",
                                asset.to_lowercase()
                            ));
                            std::fs::write(&out, log.to_json().unwrap()).unwrap();
                            println!("Shadow journal written to {}", out.display());
                        }
                        Err(e) => println!("Shadow failed: {e}"),
                    }
                }
                Err(_) => {
                    println!("Shadow: set DOCOC_LAYA_MODEL to run the model-backed loop.");
                    println!(
                        "Shadow mode: {} over {} decision(s) (placeholder, never orders)",
                        asset, bars
                    );
                    for i in 0..bars {
                        println!("  #{}: HOLD (shadow, no orders placed)", i + 1);
                    }
                }
            }
        }
        Some(Commands::Asset { action }) => match action {
            AssetAction::Add { asset } => {
                let pack = dococ_asset::AssetPack::new(&asset.to_uppercase());
                write_pack(&pack);
                println!("Asset pack added: {}", pack.to_json().unwrap());
            }
            AssetAction::List => {
                if let Ok(entries) = std::fs::read_dir(assets_dir()) {
                    for entry in entries.flatten() {
                        if let Ok(text) = std::fs::read_to_string(entry.path()) {
                            if let Ok(pack) = dococ_asset::AssetPack::from_json(&text) {
                                println!("{} stage={:?}", pack.asset, pack.stage);
                            }
                        }
                    }
                } else {
                    println!("No asset packs");
                }
            }
            AssetAction::Show { asset } => {
                match read_pack(&asset) {
                    Some(pack) => println!("{}", pack.to_json().unwrap()),
                    None => println!("Asset pack not found: {}", asset),
                }
            }
            AssetAction::Promote { asset } => {
                if let Some(mut pack) = read_pack(&asset) {
                    match pack.promote() {
                        Ok(stage) => {
                            write_pack(&pack);
                            println!("Promoted {} -> {:?}", pack.asset, stage);
                        }
                        Err(e) => println!("Promote failed: {}", e),
                    }
                } else {
                    println!("Asset pack not found: {}", asset);
                }
            }
            AssetAction::Demote { asset } => {
                if let Some(mut pack) = read_pack(&asset) {
                    match pack.demote() {
                        Ok(stage) => {
                            write_pack(&pack);
                            println!("Demoted {} -> {:?}", pack.asset, stage);
                        }
                        Err(e) => println!("Demote failed: {}", e),
                    }
                } else {
                    println!("Asset pack not found: {}", asset);
                }
            }
            AssetAction::Rm { asset } => {
                let path = assets_dir().join(format!("{}.json", asset.to_uppercase()));
                if std::fs::remove_file(path).is_ok() {
                    println!("Removed asset pack: {}", asset.to_uppercase());
                } else {
                    println!("Asset pack not found: {}", asset);
                }
            }
        },
        Some(Commands::Dash { asset }) => {
            if let Err(e) = dococ_tui::run(&asset) {
                eprintln!("Dashboard error: {e}");
            }
        }
        Some(Commands::Version) | None => {
            println!("dococ 0.1.0 (Phase 1)");
        }
    }
}
