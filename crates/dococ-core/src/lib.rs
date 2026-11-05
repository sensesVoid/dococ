use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountId(pub u64);

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Money {
    pub amount: f64,
    pub currency: &'static str,
}

impl Money {
    pub fn new(amount: f64, currency: &'static str) -> Self {
        Self { amount, currency }
    }
    pub fn usd(amount: f64) -> Self {
        Self::new(amount, "USD")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OrderType {
    Market,
    Limit { price: f64 },
    Stop { price: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderState {
    New,
    Working,
    Filled,
    Canceled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub symbol: Symbol,
    pub side: Side,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
    pub state: OrderState,
    pub filled_quantity: f64,
    pub avg_fill_price: f64,
    pub created_at: i64,
}

impl Order {
    pub fn new(id: u64, symbol: Symbol, side: Side, quantity: f64, price: Option<f64>, created_at: i64) -> Self {
        Self {
            id,
            symbol,
            side,
            order_type: if price.is_some() { OrderType::Limit { price: price.unwrap() } } else { OrderType::Market },
            quantity,
            price,
            state: OrderState::New,
            filled_quantity: 0.0,
            avg_fill_price: 0.0,
            created_at,
        }
    }

    pub fn market(id: u64, symbol: Symbol, side: Side, quantity: f64, created_at: i64) -> Self {
        Self::new(id, symbol, side, quantity, None, created_at)
    }

    pub fn is_filled(&self) -> bool {
        self.state == OrderState::Filled
    }

    pub fn remaining_quantity(&self) -> f64 {
        (self.quantity - self.filled_quantity).max(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub order_id: u64,
    pub symbol: Symbol,
    pub side: Side,
    pub quantity: f64,
    pub price: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: Symbol,
    pub quantity: f64,
    pub avg_price: f64,
}

impl Position {
    pub fn new(symbol: Symbol) -> Self {
        Self { symbol, quantity: 0.0, avg_price: 0.0 }
    }

    pub fn market_value(&self, price: f64) -> f64 {
        self.quantity * price
    }

    pub fn unrealized_pnl(&self, price: f64) -> f64 {
        (price - self.avg_price) * self.quantity
    }
}

#[derive(Debug, Clone)]
pub struct Instrument {
    pub symbol: Symbol,
    pub tick_size: f64,
    pub lot_size: f64,
    pub margin_requirement: f64,
    pub session_start_utc: Option<i64>,
    pub session_end_utc: Option<i64>,
}

impl Instrument {
    pub fn new(symbol: Symbol, tick_size: f64, lot_size: f64, margin_requirement: f64) -> Self {
        Self {
            symbol,
            tick_size,
            lot_size,
            margin_requirement,
            session_start_utc: None,
            session_end_utc: None,
        }
    }

    pub fn round_price(&self, price: f64) -> f64 {
        (price / self.tick_size).round() * self.tick_size
    }

    pub fn round_quantity(&self, qty: f64) -> f64 {
        (qty / self.lot_size).floor() * self.lot_size
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub equity: f64,
    pub cash: f64,
    pub positions: HashMap<Symbol, Position>,
    pub daily_pnl: f64,
    pub peak_equity: f64,
    pub currency: &'static str,
}

impl Account {
    pub fn new(id: AccountId, equity: f64) -> Self {
        Self {
            id,
            equity,
            cash: equity,
            positions: HashMap::new(),
            daily_pnl: 0.0,
            peak_equity: equity,
            currency: "USD",
        }
    }

    pub fn get_position(&self, symbol: &Symbol) -> Option<&Position> {
        self.positions.get(symbol)
    }

    pub fn update_peak(&mut self) {
        if self.equity > self.peak_equity {
            self.peak_equity = self.equity;
        }
    }

    pub fn drawdown(&self) -> f64 {
        if self.peak_equity > 0.0 {
            (self.peak_equity - self.equity) / self.peak_equity
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    MarketData { symbol: Symbol, bar: Bar, timestamp: i64 },
    OrderUpdate { order_id: u64, state: OrderState, filled_qty: f64, price: f64 },
    Timer { name: String, timestamp: i64 },
    LlmBrief { brief: String, timestamp: i64 },
    Tick { timestamp: i64 },
}

pub trait Clock: Send + Sync {
    fn now(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

pub struct FixedClock {
    pub time: i64,
}

impl Clock for FixedClock {
    fn now(&self) -> i64 {
        self.time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_has_ohlc_and_volume() {
        let bar = Bar {
            timestamp: 1_700_000_000,
            open: 64_000.0,
            high: 64_500.0,
            low: 63_800.0,
            close: 64_200.0,
            volume: 1_234.5,
        };
        assert_eq!(bar.timestamp, 1_700_000_000);
        assert!((bar.close - 64_200.0).abs() < 1e-9);
    }

    #[test]
    fn position_tracks_quantity_and_avg_price() {
        let pos = Position {
            symbol: Symbol::new("BTCUSDT"),
            quantity: 0.5,
            avg_price: 64_000.0,
        };
        assert_eq!(pos.symbol.to_string(), "BTCUSDT");
        assert!((pos.quantity - 0.5).abs() < 1e-9);
    }

    #[test]
    fn account_id_display() {
        let id = AccountId(42);
        assert_eq!(id.to_string(), "42");
    }

    #[test]
    fn money_usd() {
        let m = Money::usd(100.0);
        assert!((m.amount - 100.0).abs() < 1e-9);
        assert_eq!(m.currency, "USD");
    }

    #[test]
    fn instrument_rounds_price() {
        let inst = Instrument::new(Symbol::new("BTCUSDT"), 0.01, 0.001, 0.05);
        assert!((inst.round_price(64_000.005) - 64_000.01).abs() < 1e-9);
        assert!((inst.round_quantity(0.1234) - 0.123).abs() < 1e-9);
    }

    #[test]
    fn account_drawdown() {
        let mut acct = Account::new(AccountId(1), 100_000.0);
        acct.equity = 95_000.0;
        acct.update_peak();
        assert!((acct.drawdown() - 0.05).abs() < 1e-9);
    }

    #[test]
    fn fixed_clock_returns_time() {
        let clock = FixedClock { time: 1_700_000_000 };
        assert_eq!(clock.now(), 1_700_000_000);
    }

    #[test]
    fn order_market_creation() {
        let order = Order::market(1, Symbol::new("BTCUSDT"), Side::Buy, 0.1, 1_700_000_000);
        assert_eq!(order.state, OrderState::New);
        assert_eq!(order.order_type, OrderType::Market);
        assert!((order.remaining_quantity() - 0.1).abs() < 1e-9);
    }
}
