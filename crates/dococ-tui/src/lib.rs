use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Terminal,
};
use serde::Deserialize;
use std::{io, path::Path, time::Duration};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct JournalRow {
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub price: f64,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub target_qty: f64,
    #[serde(default)]
    pub position_qty: f64,
    #[serde(default)]
    pub equity: f64,
    #[serde(default)]
    pub drawdown: f64,
    #[serde(default)]
    pub oms_filled: usize,
    #[serde(default)]
    pub risk_decision: String,
}

#[derive(Debug, Clone, Default)]
pub struct DashboardState {
    pub symbol: String,
    pub equity: f64,
    pub drawdown: f64,
    pub position_qty: f64,
    pub last_price: f64,
    pub last_action: String,
    pub last_confidence: f64,
    pub decisions: usize,
    pub filled_orders: usize,
    pub regime: String,
    pub news: String,
    pub health: String,
    pub recent: Vec<JournalRow>,
}

fn load_journal(symbol: &str) -> Vec<JournalRow> {
    let path = format!("paper_{}.json", symbol.to_lowercase());
    let text = match std::fs::read_to_string(Path::new(&path)) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<Vec<JournalRow>>(&text).unwrap_or_default()
}

fn state_from_journal(symbol: &str, rows: &[JournalRow]) -> DashboardState {
    let mut s = DashboardState {
        symbol: symbol.to_uppercase(),
        regime: "unknown (LLM brief not wired)".to_string(),
        news: "no feed (skeleton)".to_string(),
        health: "paper skeleton · journal source".to_string(),
        decisions: rows.len(),
        ..Default::default()
    };
    if let Some(last) = rows.last() {
        s.equity = last.equity;
        s.drawdown = last.drawdown;
        s.position_qty = last.position_qty;
        s.last_price = last.price;
        s.last_action = last.action.clone();
        s.last_confidence = last.confidence;
        s.filled_orders = last.oms_filled;
    }
    let take = rows.len().saturating_sub(12);
    s.recent = rows[take..].to_vec();
    s
}

/// Run the dashboard skeleton TUI. Press `q` or `Esc` to quit.
pub fn run(symbol: &str) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let rows = load_journal(symbol);
    let state = state_from_journal(symbol, &rows);
    let res = run_loop(&mut terminal, &state);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &DashboardState,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, state))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        _ => {}
                    }
                }
            }
        }
    }
}

fn draw(f: &mut ratatui::Frame, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(f.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" Doc Oc ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!("dashboard skeleton · {} · ", state.symbol)),
        Span::raw(&state.health),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(header, chunks[0]);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let acct = Paragraph::new(vec![
        Line::from(format!("Equity:      {:.2}", state.equity)),
        Line::from(format!("Drawdown:    {:.2}%", state.drawdown * 100.0)),
        Line::from(format!("Position:    {:.4}", state.position_qty)),
        Line::from(format!("Last price:  {:.2}", state.last_price)),
        Line::from(format!(
            "Last action: {} ({:.3})",
            if state.last_action.is_empty() {
                "n/a"
            } else {
                &state.last_action
            },
            state.last_confidence
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("Account / PnL"))
    .wrap(Wrap { trim: true });
    f.render_widget(acct, top[0]);

    let intel = Paragraph::new(vec![
        Line::from(format!("Regime:  {}", state.regime)),
        Line::from(format!("News:    {}", state.news)),
        Line::from(format!("Decisions: {}  Filled orders: {}", state.decisions, state.filled_orders)),
        Line::from("Sharpe/win-rate: (needs history — skeleton)"),
        Line::from("System: daemon IPC not wired; reading paper_*.json"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Regime / News / Health"),
    )
    .wrap(Wrap { trim: true });
    f.render_widget(intel, top[1]);

    render_decisions(f, chunks[2], state);

    let footer = Paragraph::new("q/Esc: quit · skeleton: no daemon stream, no live risk controls")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[3]);
}

fn render_decisions(f: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    let header = Row::new(vec!["ts", "action", "conf", "target", "pos", "equity", "risk"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    let rows = state.recent.iter().map(|r| {
        Row::new(vec![
            Cell::from(r.timestamp.to_string()),
            Cell::from(r.action.clone()),
            Cell::from(format!("{:.3}", r.confidence)),
            Cell::from(format!("{:.4}", r.target_qty)),
            Cell::from(format!("{:.4}", r.position_qty)),
            Cell::from(format!("{:.0}", r.equity)),
            Cell::from(r.risk_decision.clone()),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Recent Decisions (paper journal)"),
    );
    f.render_widget(table, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_from_empty_journal() {
        let s = state_from_journal("BTCUSDT", &[]);
        assert_eq!(s.symbol, "BTCUSDT");
        assert_eq!(s.decisions, 0);
    }

    #[test]
    fn state_from_journal_uses_last() {
        let rows = vec![
            JournalRow {
                equity: 100.0,
                action: "Long".into(),
                ..Default::default()
            },
            JournalRow {
                equity: 101.0,
                action: "Close".into(),
                confidence: 0.5,
                drawdown: 0.01,
                ..Default::default()
            },
        ];
        let s = state_from_journal("BTCUSDT", &rows);
        assert_eq!(s.equity, 101.0);
        assert_eq!(s.last_action, "Close");
        assert_eq!(s.decisions, 2);
    }
}
