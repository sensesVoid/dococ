use clap::{Parser, Subcommand};

struct DummyPaperStrategy;
impl dococ_engine::Strategy for DummyPaperStrategy {
    fn name(&self) -> &str {
        "dummy-paper"
    }
    fn decide(
        &mut self,
        _state: &str,
    ) -> Result<(dococ_laya::OrderAction, f64), Box<dyn std::error::Error>> {
        Ok((dococ_laya::OrderAction::Hold, 0.5))
    }
}

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
        /// Number of recent live bars to fetch
        #[arg(default_value_t = 50)]
        bars: usize,
        /// Keep polling and appending (idempotent by timestamp)
        #[arg(long)]
        r#loop: bool,
        /// Poll interval in seconds when --loop is set
        #[arg(long, default_value_t = 300)]
        interval: u64,
        /// Ignore saved sim state and start fresh
        #[arg(long)]
        fresh: bool,
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
    /// Print metrics report from a paper journal
    Report {
        /// Asset symbol (reads paper_<asset>.json)
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
        Some(Commands::Paper {
            asset,
            bars: n_bars,
            r#loop,
            interval,
            fresh,
        }) => {
            let model_dir = std::env::var("DOCOC_LAYA_MODEL")
                .unwrap_or_else(|_| "models/btcusdt_ml".to_string());
            let symbol = asset.to_uppercase();
            let out = std::path::PathBuf::from(format!("paper_{}.json", symbol.to_lowercase()));
            let state_path =
                std::path::PathBuf::from(format!("paper_{}.state.json", symbol.to_lowercase()));
            println!(
                "Paper trading: {} (model: {}, bars: {}, loop: {}, interval: {}s, fresh: {})",
                symbol, model_dir, n_bars, r#loop, interval, fresh
            );
            let dry = std::env::var("DOCOC_PAPER_DRY").ok().as_deref() == Some("1");
            let strategy_result: Result<Box<dyn dococ_engine::Strategy>, String> = if dry {
                println!("Paper DRY mode: using dummy strategy (no model load)");
                Ok(Box::new(DummyPaperStrategy))
            } else {
                dococ_laya::LayaRuntime::load(&model_dir)
                    .map(|laya| {
                        Box::new(dococ_engine::LayaStrategy::new(laya)) as Box<dyn dococ_engine::Strategy>
                    })
                    .map_err(|e| e.to_string())
            };
            match strategy_result {
                Ok(strategy) => {
                    let slippage_bps = 5.0;
                    // Try to restore sim state for cross-process continuity.
                    let restored = if !fresh {
                        std::fs::read_to_string(&state_path)
                            .ok()
                            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    } else {
                        None
                    };
                    let (venue, mut account, restored_ok) = if let Some(st) = restored {
                        let cash = st["cash"].as_f64().unwrap_or(100_000.0);
                        let next_id = st["next_order_id"].as_u64().unwrap_or(1);
                        let peak = st["account_peak_equity"].as_f64().unwrap_or(100_000.0);
                        let mut positions = std::collections::HashMap::new();
                        let mut prices = std::collections::HashMap::new();
                        if let Some(pos_obj) = st["positions"].as_object() {
                            for (sym, p) in pos_obj {
                                positions.insert(
                                    sym.clone(),
                                    dococ_venue::Position {
                                        symbol: sym.clone(),
                                        quantity: p["quantity"].as_f64().unwrap_or(0.0),
                                        avg_price: p["avg_price"].as_f64().unwrap_or(0.0),
                                    },
                                );
                            }
                        }
                        if let Some(pr_obj) = st["prices"].as_object() {
                            for (sym, p) in pr_obj {
                                if let Some(v) = p.as_f64() {
                                    prices.insert(sym.clone(), v);
                                }
                            }
                        }
                        let venue =
                            dococ_venue::SimVenue::restore(cash, slippage_bps, positions.clone(), prices, next_id);
                        let mut account =
                            dococ_core::Account::new(dococ_core::AccountId(1), 100_000.0);
                        account.peak_equity = peak;
                        for (sym, p) in &positions {
                            if p.quantity.abs() > 1e-12 {
                                let cs = dococ_core::Symbol::new(sym);
                                let mut cp = dococ_core::Position::new(cs.clone());
                                cp.quantity = p.quantity;
                                cp.avg_price = p.avg_price;
                                account.positions.insert(cs, cp);
                            }
                        }
                        account.equity = venue.equity_mtm();
                        account.update_peak();
                        println!("Restored sim state from {}", state_path.display());
                        (venue, account, true)
                    } else {
                        if fresh && (out.exists() || state_path.exists()) {
                            let _ = std::fs::rename(&out, out.with_extension("json.bak"));
                            let _ = std::fs::rename(&state_path, state_path.with_extension("json.bak"));
                            println!("Backed up previous paper journal/state (--fresh)");
                        } else if !fresh && (out.exists() || state_path.exists()) {
                            // No valid state but artifacts exist: start clean session to avoid
                            // inconsistent replay; preserve artifacts.
                            let _ = std::fs::rename(&out, out.with_extension("json.bak"));
                            let _ = std::fs::rename(&state_path, state_path.with_extension("json.bak"));
                            println!("No valid state file; backed up artifacts and starting fresh session");
                        }
                        let venue = dococ_venue::SimVenue::new(100_000.0, slippage_bps);
                        let account = dococ_core::Account::new(dococ_core::AccountId(1), 100_000.0);
                        (venue, account, false)
                    };
                    let scheduler = dococ_engine::Scheduler::new(900, 1);
                    let mut engine = dococ_engine::Engine::new(
                        strategy,
                        dococ_risk::RiskConfig::default(),
                        venue,
                        account,
                        scheduler,
                    );
                    let sym = dococ_core::Symbol::new(&symbol);
                    // Load journal for idempotency (append when restored; fresh session starts empty).
                    let mut journal: Vec<serde_json::Value> = if restored_ok {
                        std::fs::read_to_string(&out)
                            .ok()
                            .and_then(|t| serde_json::from_str(&t).ok())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let mut seen: std::collections::HashSet<i64> = journal
                        .iter()
                        .filter_map(|v| v.get("timestamp").and_then(|t| t.as_i64()))
                        .collect();
                    if restored_ok && !journal.is_empty() {
                        println!("Loaded {} journal row(s) (idempotent resume)", journal.len());
                    }
                    loop {
                        let bars = dococ_venue::live::fetch_latest(&symbol, n_bars).unwrap_or_default();
                        if bars.is_empty() {
                            println!("No live bars fetched.");
                        }
                        let mut ticks = 0usize;
                        for (ts, close) in &bars {
                            if seen.contains(ts) {
                                continue;
                            }
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
                                seen.insert(*ts);
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
                        let _ = std::fs::write(
                            &out,
                            serde_json::to_string_pretty(&journal).unwrap_or_else(|_| "[]".to_string()),
                        );
                        // Persist sim state for cross-process resume.
                        let mut pos_map = serde_json::Map::new();
                        let mut price_map = serde_json::Map::new();
                        for (s, p) in dococ_venue::Venue::positions(engine.venue()) {
                            pos_map.insert(
                                s.clone(),
                                serde_json::json!({"quantity": p.quantity, "avg_price": p.avg_price}),
                            );
                        }
                        // Persist last known price for the traded symbol.
                        if let Some(last) = bars.last() {
                            price_map.insert(symbol.clone(), serde_json::json!(last.1));
                        }
                        let st = serde_json::json!({
                            "symbol": symbol,
                            "cash": engine.venue().cash(),
                            "slippage_bps": slippage_bps,
                            "next_order_id": engine.venue().next_order_id(),
                            "positions": pos_map,
                            "prices": price_map,
                            "account_peak_equity": acct.peak_equity,
                        });
                        let _ = std::fs::write(
                            &state_path,
                            serde_json::to_string_pretty(&st).unwrap_or_else(|_| "{}".to_string()),
                        );
                        println!(
                            "Paper batch: +{} tick(s), total {}, equity {:.2}, DD {:.2}%, fills {}, journal {}, state {}",
                            ticks,
                            journal.len(),
                            acct.equity,
                            acct.drawdown() * 100.0,
                            engine.oms().filled_orders().len(),
                            out.display(),
                            state_path.display()
                        );
                        if !r#loop {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(interval));
                    }
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
        Some(Commands::Report { asset }) => {
            let symbol = asset.to_uppercase();
            let path = format!("paper_{}.json", symbol.to_lowercase());
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => {
                    println!("No paper journal found: {}", path);
                    return;
                }
            };
            let rows: Vec<dococ_tui::JournalRow> =
                serde_json::from_str(&text).unwrap_or_default();
            let m = dococ_tui::compute_metrics(&rows);
            let (equity, dd, pos) = rows
                .last()
                .map(|r| (r.equity, r.drawdown, r.position_qty))
                .unwrap_or((0.0, 0.0, 0.0));
            println!("Doc Oc paper report: {}", symbol);
            println!("  Decisions:    {}", rows.len());
            println!("  Trades:       {}", m.trades);
            println!("  Equity:       {:.2}", equity);
            println!("  Position:     {:.4}", pos);
            println!("  Return:       {:+.2}%", m.total_return_pct);
            println!("  Max drawdown: {:.2}%", m.max_drawdown_pct);
            println!("  Drawdown now: {:.2}%", dd * 100.0);
            println!("  Sharpe (ann.): {:.2}", m.sharpe);
            println!("  Win rate:     {:.1}%", m.win_rate);
            println!("  Avg conf:     {:.3}", m.avg_confidence);
            println!("  Journal:      {}", path);
        }
        Some(Commands::Version) | None => {
            println!("dococ 0.1.0 (Phase 1)");
        }
    }
}
