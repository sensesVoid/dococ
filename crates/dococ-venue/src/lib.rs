use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OrderType {
    Market,
    Limit { price: f64 },
}

#[derive(Debug, Clone)]
pub struct Order {
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub order_type: OrderType,
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub order_id: u64,
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub price: f64,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub avg_price: f64,
}

pub trait Venue {
    fn submit(&mut self, order: Order) -> Result<Fill, &'static str>;
    fn positions(&self) -> &HashMap<String, Position>;
    fn equity(&self) -> f64;
}

pub struct SimVenue {
    equity: f64,
    slippage_bps: f64,
    prices: HashMap<String, f64>,
    positions: HashMap<String, Position>,
    next_order_id: u64,
}

impl SimVenue {
    pub fn new(equity: f64, slippage_bps: f64) -> Self {
        Self {
            equity,
            slippage_bps,
            prices: HashMap::new(),
            positions: HashMap::new(),
            next_order_id: 1,
        }
    }

    pub fn set_price(&mut self, symbol: &str, price: f64) {
        self.prices.insert(symbol.to_string(), price);
    }

    fn fill_price(&self, side: Side, price: f64) -> f64 {
        let slip = self.slippage_bps / 10_000.0;
        match side {
            Side::Buy => price * (1.0 + slip),
            Side::Sell => price * (1.0 - slip),
        }
    }
}

impl Venue for SimVenue {
    fn submit(&mut self, order: Order) -> Result<Fill, &'static str> {
        let market_price = self
            .prices
            .get(&order.symbol)
            .copied()
            .ok_or("no price set for symbol")?;

        let fill_price = match order.order_type {
            OrderType::Market => self.fill_price(order.side, market_price),
            OrderType::Limit { price } => {
                match order.side {
                    Side::Buy if market_price <= price => self.fill_price(order.side, market_price),
                    Side::Sell if market_price >= price => self.fill_price(order.side, market_price),
                    _ => return Err("limit not fillable"),
                }
            }
        };

        let order_id = self.next_order_id;
        self.next_order_id += 1;

        let fill = Fill {
            order_id,
            symbol: order.symbol.clone(),
            side: order.side,
            quantity: order.quantity,
            price: fill_price,
        };

        let position = self.positions.entry(order.symbol.clone()).or_insert(Position {
            symbol: order.symbol.clone(),
            quantity: 0.0,
            avg_price: 0.0,
        });

        let signed_qty = match order.side {
            Side::Buy => order.quantity,
            Side::Sell => -order.quantity,
        };

        if position.quantity == 0.0 {
            position.avg_price = fill_price;
        } else {
            let total_qty = position.quantity + signed_qty;
            if total_qty != 0.0 {
                position.avg_price = (position.avg_price * position.quantity
                    + fill_price * signed_qty)
                    / total_qty;
            }
        }
        position.quantity += signed_qty;

        let cost = fill_price * order.quantity;
        match order.side {
            Side::Buy => self.equity -= cost,
            Side::Sell => self.equity += cost,
        }

        Ok(fill)
    }

    fn positions(&self) -> &HashMap<String, Position> {
        &self.positions
    }

    fn equity(&self) -> f64 {
        self.equity
    }
}

/// Cost model for trading fees and slippage.
#[derive(Debug, Clone, Copy)]
pub struct CostModel {
    pub fee_bps: f64,
    pub slippage_bps: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            fee_bps: 5.0,
            slippage_bps: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CostEstimate {
    pub fee: f64,
    pub slippage: f64,
    pub total: f64,
}

impl CostModel {
    pub fn estimate(&self, quantity: f64, price: f64, fee_bps: f64, slippage_bps: f64) -> CostEstimate {
        let notional = quantity * price;
        let fee = notional * fee_bps / 10_000.0;
        let slippage = notional * slippage_bps / 10_000.0;
        CostEstimate {
            fee,
            slippage,
            total: fee + slippage,
        }
    }

    pub fn estimate_default(&self, quantity: f64, price: f64) -> CostEstimate {
        self.estimate(quantity, price, self.fee_bps, self.slippage_bps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_venue_fills_market_buy_with_slippage() {
        let mut venue = SimVenue::new(100_000.0, 5.0);
        venue.set_price("BTCUSDT", 64_000.0);
        let order = Order {
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.1,
            order_type: OrderType::Market,
        };
        let fill = venue.submit(order).expect("fill");
        assert_eq!(fill.symbol, "BTCUSDT");
        assert_eq!(fill.side, Side::Buy);
        assert!((fill.quantity - 0.1).abs() < 1e-9);
        assert!((fill.price - 64_032.0).abs() < 1e-6);
    }

    #[test]
    fn sim_venue_rejects_unknown_symbol() {
        let mut venue = SimVenue::new(100_000.0, 5.0);
        let order = Order {
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.1,
            order_type: OrderType::Market,
        };
        assert!(venue.submit(order).is_err());
    }

    #[test]
    fn sim_venue_tracks_position() {
        let mut venue = SimVenue::new(100_000.0, 5.0);
        venue.set_price("BTCUSDT", 64_000.0);
        let buy = Order {
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.1,
            order_type: OrderType::Market,
        };
        venue.submit(buy).expect("buy");
        let pos = venue.positions().get("BTCUSDT").expect("position");
        assert!((pos.quantity - 0.1).abs() < 1e-9);
    }

    #[test]
    fn sim_venue_sell_reduces_position() {
        let mut venue = SimVenue::new(100_000.0, 5.0);
        venue.set_price("BTCUSDT", 64_000.0);
        let buy = Order {
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.1,
            order_type: OrderType::Market,
        };
        venue.submit(buy).expect("buy");
        let sell = Order {
            symbol: "BTCUSDT".into(),
            side: Side::Sell,
            quantity: 0.05,
            order_type: OrderType::Market,
        };
        venue.submit(sell).expect("sell");
        let pos = venue.positions().get("BTCUSDT").expect("position");
        assert!((pos.quantity - 0.05).abs() < 1e-9);
    }

    #[test]
    fn cost_model_computes_fee() {
        let model = CostModel::default();
        let cost = model.estimate(1.0, 64_000.0, 5.0, 1.0);
        assert!((cost.total - 38.4).abs() < 1e-6);
    }

    #[test]
    fn cost_model_includes_slippage() {
        let model = CostModel::default();
        let cost = model.estimate(1.0, 64_000.0, 5.0, 2.0);
        assert!((cost.total - 44.8).abs() < 1e-6);
        assert!((cost.fee - 32.0).abs() < 1e-6);
        assert!((cost.slippage - 12.8).abs() < 1e-6);
    }

    #[test]
    fn cost_model_scales_with_size() {
        let model = CostModel::default();
        let cost1 = model.estimate(1.0, 64_000.0, 5.0, 1.0);
        let cost2 = model.estimate(2.0, 64_000.0, 5.0, 1.0);
        assert!((cost2.total - cost1.total * 2.0).abs() < 1e-6);
    }
}
