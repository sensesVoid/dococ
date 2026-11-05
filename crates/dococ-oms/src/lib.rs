#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
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
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub price: f64,
    pub state: OrderState,
    pub filled_quantity: f64,
    pub avg_fill_price: f64,
}

impl Order {
    pub fn new(id: u64, symbol: &str, side: Side, quantity: f64, price: f64) -> Self {
        Self {
            id,
            symbol: symbol.to_string(),
            side,
            quantity,
            price,
            state: OrderState::New,
            filled_quantity: 0.0,
            avg_fill_price: 0.0,
        }
    }

    pub fn mark_working(&mut self) {
        if self.state == OrderState::New {
            self.state = OrderState::Working;
        }
    }

    pub fn fill(&mut self, quantity: f64, price: f64) {
        if self.state != OrderState::Working {
            return;
        }
        let new_filled = self.filled_quantity + quantity;
        if new_filled >= self.quantity - 1e-9 {
            self.state = OrderState::Filled;
            self.filled_quantity = self.quantity;
        } else {
            self.filled_quantity = new_filled;
        }
        // Update average fill price.
        if self.filled_quantity > 0.0 {
            self.avg_fill_price = (self.avg_fill_price * (self.filled_quantity - quantity)
                + price * quantity)
                / self.filled_quantity;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == OrderState::Working || self.state == OrderState::New {
            self.state = OrderState::Canceled;
        }
    }

    pub fn reject(&mut self) {
        self.state = OrderState::Rejected;
    }

    pub fn is_filled(&self) -> bool {
        self.state == OrderState::Filled
    }

    pub fn remaining_quantity(&self) -> f64 {
        (self.quantity - self.filled_quantity).max(0.0)
    }
}

pub struct OrderManager {
    orders: Vec<Order>,
    next_id: u64,
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            orders: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_order(&mut self, symbol: &str, side: Side, quantity: f64, price: f64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.orders.push(Order::new(id, symbol, side, quantity, price));
        id
    }

    pub fn get(&self, id: u64) -> Option<&Order> {
        self.orders.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Order> {
        self.orders.iter_mut().find(|o| o.id == id)
    }

    pub fn working_orders(&self) -> Vec<&Order> {
        self.orders
            .iter()
            .filter(|o| o.state == OrderState::Working)
            .collect()
    }

    pub fn filled_orders(&self) -> Vec<&Order> {
        self.orders
            .iter()
            .filter(|o| o.state == OrderState::Filled)
            .collect()
    }

    pub fn cancel_all(&mut self) {
        for order in &mut self.orders {
            if order.state == OrderState::Working || order.state == OrderState::New {
                order.state = OrderState::Canceled;
            }
        }
    }
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_order_starts_in_new_state() {
        let order = Order::new(1, "BTCUSDT", Side::Buy, 0.1, 64_000.0);
        assert_eq!(order.state, OrderState::New);
    }

    #[test]
    fn order_transitions_to_working() {
        let mut order = Order::new(1, "BTCUSDT", Side::Buy, 0.1, 64_000.0);
        order.mark_working();
        assert_eq!(order.state, OrderState::Working);
    }

    #[test]
    fn order_transitions_to_filled() {
        let mut order = Order::new(1, "BTCUSDT", Side::Buy, 0.1, 64_000.0);
        order.mark_working();
        order.fill(0.1, 64_032.0);
        assert_eq!(order.state, OrderState::Filled);
        assert!((order.filled_quantity - 0.1).abs() < 1e-9);
        assert!((order.avg_fill_price - 64_032.0).abs() < 1e-6);
    }

    #[test]
    fn partial_fill_stays_working() {
        let mut order = Order::new(1, "BTCUSDT", Side::Buy, 0.1, 64_000.0);
        order.mark_working();
        order.fill(0.05, 64_032.0);
        assert_eq!(order.state, OrderState::Working);
        assert!((order.filled_quantity - 0.05).abs() < 1e-9);
    }

    #[test]
    fn cancel_working_order() {
        let mut order = Order::new(1, "BTCUSDT", Side::Buy, 0.1, 64_000.0);
        order.mark_working();
        order.cancel();
        assert_eq!(order.state, OrderState::Canceled);
    }

    #[test]
    fn order_manager_creates_and_tracks() {
        let mut mgr = OrderManager::new();
        let id = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0);
        assert!(mgr.get(id).is_some());
        assert_eq!(mgr.get(id).unwrap().state, OrderState::New);
    }

    #[test]
    fn order_manager_cancel_all() {
        let mut mgr = OrderManager::new();
        let id1 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0);
        let id2 = mgr.create_order("BTCUSDT", Side::Sell, 0.05, 64_000.0);
        if let Some(o) = mgr.get_mut(id1) {
            o.mark_working();
        }
        if let Some(o) = mgr.get_mut(id2) {
            o.mark_working();
        }
        mgr.cancel_all();
        assert_eq!(mgr.get(id1).unwrap().state, OrderState::Canceled);
        assert_eq!(mgr.get(id2).unwrap().state, OrderState::Canceled);
    }
}
