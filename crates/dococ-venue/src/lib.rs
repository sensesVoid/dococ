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

#[derive(Debug, Clone)]
pub struct StopOrder {
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub stop_price: f64,
}

pub struct SimVenue {
    equity: f64,
    slippage_bps: f64,
    prices: HashMap<String, f64>,
    positions: HashMap<String, Position>,
    next_order_id: u64,
    stop_orders: Vec<StopOrder>,
}

impl SimVenue {
    pub fn new(equity: f64, slippage_bps: f64) -> Self {
        Self {
            equity,
            slippage_bps,
            prices: HashMap::new(),
            positions: HashMap::new(),
            next_order_id: 1,
            stop_orders: Vec::new(),
        }
    }

    pub fn set_price(&mut self, symbol: &str, price: f64) {
        self.prices.insert(symbol.to_string(), price);
    }

    pub fn set_stop(&mut self, symbol: &str, side: Side, quantity: f64, stop_price: f64) {
        self.stop_orders.push(StopOrder {
            symbol: symbol.to_string(),
            side,
            quantity,
            stop_price,
        });
    }

    pub fn cancel_stops(&mut self, symbol: &str) {
        self.stop_orders.retain(|s| s.symbol != symbol);
    }

    pub fn check_stops(&mut self, symbol: &str, current_price: f64) -> Vec<Fill> {
        let mut fills = Vec::new();
        let mut triggered = Vec::new();

        for (i, stop) in self.stop_orders.iter().enumerate() {
            if stop.symbol != symbol {
                continue;
            }
            let trigger = match stop.side {
                Side::Sell => current_price <= stop.stop_price,
                Side::Buy => current_price >= stop.stop_price,
            };
            if trigger {
                triggered.push(i);
            }
        }

        // Process triggered stops (in reverse order to maintain indices).
        for &i in triggered.iter().rev() {
            let stop = self.stop_orders.remove(i);
            let fill_price = self.fill_price(stop.side, current_price);
            let order_id = self.next_order_id;
            self.next_order_id += 1;

            let fill = Fill {
                order_id,
                symbol: stop.symbol.clone(),
                side: stop.side,
                quantity: stop.quantity,
                price: fill_price,
            };

            // Update position.
            let position = self
                .positions
                .entry(stop.symbol.clone())
                .or_insert(Position {
                    symbol: stop.symbol.clone(),
                    quantity: 0.0,
                    avg_price: 0.0,
                });

            let signed_qty = match stop.side {
                Side::Buy => stop.quantity,
                Side::Sell => -stop.quantity,
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

            let cost = fill_price * stop.quantity;
            match stop.side {
                Side::Buy => self.equity -= cost,
                Side::Sell => self.equity += cost,
            }

            fills.push(fill);
        }

        fills
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
            OrderType::Limit { price } => match order.side {
                Side::Buy if market_price <= price => self.fill_price(order.side, market_price),
                Side::Sell if market_price >= price => self.fill_price(order.side, market_price),
                _ => return Err("limit not fillable"),
            },
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

        let position = self
            .positions
            .entry(order.symbol.clone())
            .or_insert(Position {
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
                position.avg_price =
                    (position.avg_price * position.quantity + fill_price * signed_qty) / total_qty;
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

impl SimVenue {
    /// Mark-to-market equity: cash balance plus open positions valued at
    /// current marks (falls back to avg_price when no mark is set).
    /// Note: the base `equity` field is debited/credited by full notional on
    /// fills (spot-style cash accounting); this adds position mark value to
    /// produce an MTM view.
    pub fn equity_mtm(&self) -> f64 {
        let mut total = self.equity;
        for (sym, pos) in &self.positions {
            if pos.quantity.abs() < 1e-12 {
                continue;
            }
            let mark = self.prices.get(sym).copied().unwrap_or(pos.avg_price);
            total += pos.quantity * mark;
        }
        total
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
    pub fn estimate(
        &self,
        quantity: f64,
        price: f64,
        fee_bps: f64,
        slippage_bps: f64,
    ) -> CostEstimate {
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

/// Dynamic cost model recalibrating slippage by order size, volatility, and time-of-day.
#[derive(Debug, Clone, Copy)]
pub struct DynamicCostModel {
    pub base_fee_bps: f64,
    pub base_slippage_bps: f64,
    pub size_coef_bps_per_10k: f64,
    pub vol_coef: f64,
}

impl Default for DynamicCostModel {
    fn default() -> Self {
        Self {
            base_fee_bps: 5.0,
            base_slippage_bps: 1.0,
            size_coef_bps_per_10k: 0.5,
            vol_coef: 1.0,
        }
    }
}

impl DynamicCostModel {
    pub fn effective_slippage_bps(
        &self,
        notional: f64,
        vol: Option<f64>,
        is_off_hours: bool,
    ) -> f64 {
        let size_penalty = (notional / 10_000.0).max(0.0) * self.size_coef_bps_per_10k;
        let vol_factor = match vol {
            Some(v) => (v / 0.02).max(0.5).min(5.0) * self.vol_coef,
            None => 1.0,
        };
        let time_penalty = if is_off_hours { 1.5 } else { 1.0 };
        (self.base_slippage_bps + size_penalty) * vol_factor * time_penalty
    }

    pub fn estimate_dynamic(
        &self,
        quantity: f64,
        price: f64,
        vol: Option<f64>,
        is_off_hours: bool,
    ) -> CostEstimate {
        let notional = quantity * price;
        let fee = notional * self.base_fee_bps / 10_000.0;
        let slip_bps = self.effective_slippage_bps(notional, vol, is_off_hours);
        let slippage = notional * slip_bps / 10_000.0;
        CostEstimate {
            fee,
            slippage,
            total: fee + slippage,
        }
    }
}

/// Record of an executed fill with arrival context.
#[derive(Debug, Clone)]
pub struct FillRecord {
    pub fill: Fill,
    pub arrival_price: f64,
    pub fee_paid: f64,
    pub timestamp: i64,
}

/// Realized cost breakdown from actual fill execution.
#[derive(Debug, Clone, Copy)]
pub struct RealizedCost {
    pub fee_paid: f64,
    pub slippage_dollars: f64,
    pub slippage_bps: f64,
    pub total_cost: f64,
}

impl RealizedCost {
    pub fn from_fill(record: &FillRecord) -> Self {
        let price_diff = match record.fill.side {
            Side::Buy => record.fill.price - record.arrival_price,
            Side::Sell => record.arrival_price - record.fill.price,
        };
        let slippage_dollars = price_diff.max(0.0) * record.fill.quantity;
        let slippage_bps = if record.arrival_price > 0.0 {
            (price_diff.max(0.0) / record.arrival_price) * 10_000.0
        } else {
            0.0
        };
        Self {
            fee_paid: record.fee_paid,
            slippage_dollars,
            slippage_bps,
            total_cost: record.fee_paid + slippage_dollars,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_cost_model_scales_with_size_volatility_and_time() {
        let model = DynamicCostModel {
            base_fee_bps: 5.0,
            base_slippage_bps: 1.0,
            size_coef_bps_per_10k: 0.5,
            vol_coef: 1.0,
        };

        // Standard 10k trade, normal vol (0.02), regular hours
        let slip_std = model.effective_slippage_bps(10_000.0, Some(0.02), false);
        // (1.0 + 0.5) * 1.0 * 1.0 = 1.5 bps
        assert!((slip_std - 1.5).abs() < 1e-6);

        // High vol trade (0.04 -> vol factor 2.0)
        let slip_high_vol = model.effective_slippage_bps(10_000.0, Some(0.04), false);
        // 1.5 * 2.0 = 3.0 bps
        assert!((slip_high_vol - 3.0).abs() < 1e-6);

        // Off hours (penalty 1.5x)
        let slip_off_hours = model.effective_slippage_bps(10_000.0, Some(0.02), true);
        // 1.5 * 1.5 = 2.25 bps
        assert!((slip_off_hours - 2.25).abs() < 1e-6);
    }

    #[test]
    fn realized_cost_from_fills_calculates_slippage_and_fee() {
        let fill = Fill {
            order_id: 1,
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.5,
            price: 60_050.0, // Filled at 60,050 with arrival price 60,000 -> 50 slippage
        };
        let record = FillRecord {
            fill,
            arrival_price: 60_000.0,
            fee_paid: 15.0,
            timestamp: 1000,
        };

        let realized = RealizedCost::from_fill(&record);
        assert_eq!(realized.fee_paid, 15.0);
        assert!((realized.slippage_dollars - 25.0).abs() < 1e-6); // 50 * 0.5 = 25.0
        assert!((realized.total_cost - 40.0).abs() < 1e-6);
        // Slippage bps: 50 / 60_000 * 10_000 = 8.333 bps
        assert!((realized.slippage_bps - 8.3333).abs() < 1e-3);
    }

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

    #[test]
    fn stop_loss_triggers_on_price_drop() {
        let mut venue = SimVenue::new(100_000.0, 5.0);
        venue.set_price("BTCUSDT", 64_000.0);
        let buy = Order {
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.1,
            order_type: OrderType::Market,
        };
        venue.submit(buy).expect("buy");
        venue.set_stop("BTCUSDT", Side::Sell, 0.1, 62_000.0);
        let fills = venue.check_stops("BTCUSDT", 61_000.0);
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].side, Side::Sell);
    }

    #[test]
    fn stop_loss_does_not_trigger_above_stop() {
        let mut venue = SimVenue::new(100_000.0, 5.0);
        venue.set_price("BTCUSDT", 64_000.0);
        venue.set_stop("BTCUSDT", Side::Sell, 0.1, 62_000.0);
        let fills = venue.check_stops("BTCUSDT", 63_000.0);
        assert!(fills.is_empty());
    }

    #[test]
    fn cancel_stop_removes_it() {
        let mut venue = SimVenue::new(100_000.0, 5.0);
        venue.set_price("BTCUSDT", 64_000.0);
        venue.set_stop("BTCUSDT", Side::Sell, 0.1, 62_000.0);
        venue.cancel_stops("BTCUSDT");
        let fills = venue.check_stops("BTCUSDT", 61_000.0);
        assert!(fills.is_empty());
    }
}

pub mod live;

