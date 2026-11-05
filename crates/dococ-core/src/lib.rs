use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub avg_price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountId(pub u64);

impl fmt::Display for AccountId {
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
            symbol: "BTCUSDT".into(),
            quantity: 0.5,
            avg_price: 64_000.0,
        };
        assert_eq!(pos.symbol, "BTCUSDT");
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
}
