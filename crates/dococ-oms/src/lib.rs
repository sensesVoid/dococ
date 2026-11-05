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
    pub idempotency_key: Option<String>,
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
            idempotency_key: None,
        }
    }

    pub fn with_key(mut self, key: String) -> Self {
        self.idempotency_key = Some(key);
        self
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
    key_index: std::collections::HashMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReconcileIssue {
    QuantityMismatch {
        symbol: String,
        oms_qty: f64,
        venue_qty: f64,
    },
    MissingVenuePosition {
        symbol: String,
        oms_qty: f64,
    },
    UntrackedVenuePosition {
        symbol: String,
        venue_qty: f64,
    },
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            orders: Vec::new(),
            next_id: 1,
            key_index: std::collections::HashMap::new(),
        }
    }

    pub fn create_order(
        &mut self,
        symbol: &str,
        side: Side,
        quantity: f64,
        price: f64,
        idempotency_key: Option<&str>,
    ) -> u64 {
        // Check for existing order with the same key.
        if let Some(key) = idempotency_key {
            if let Some(&existing_id) = self.key_index.get(key) {
                return existing_id;
            }
        }

        let id = self.next_id;
        self.next_id += 1;
        let mut order = Order::new(id, symbol, side, quantity, price);
        if let Some(key) = idempotency_key {
            order.idempotency_key = Some(key.to_string());
            self.key_index.insert(key.to_string(), id);
        }
        self.orders.push(order);
        id
    }

    pub fn get(&self, id: u64) -> Option<&Order> {
        self.orders.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Order> {
        self.orders.iter_mut().find(|o| o.id == id)
    }

    pub fn find_by_key(&self, key: &str) -> Option<&Order> {
        self.key_index
            .get(key)
            .and_then(|&id| self.orders.iter().find(|o| o.id == id))
    }

    pub fn orders(&self) -> &[Order] {
        &self.orders
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

    /// Reconcile OMS tracked positions against venue positions.
    /// Returns a list of discrepancies.
    pub fn reconcile_positions(
        &self,
        venue_positions: &std::collections::HashMap<String, f64>,
    ) -> Vec<ReconcileIssue> {
        let mut issues = Vec::new();

        // Compute OMS position quantities by symbol.
        let mut oms_positions: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        for order in &self.orders {
            if order.state == OrderState::Filled {
                let signed = match order.side {
                    Side::Buy => order.filled_quantity,
                    Side::Sell => -order.filled_quantity,
                };
                *oms_positions.entry(order.symbol.clone()).or_insert(0.0) += signed;
            }
        }

        // Check OMS positions against venue.
        for (symbol, &oms_qty) in &oms_positions {
            match venue_positions.get(symbol) {
                Some(&venue_qty) => {
                    if (oms_qty - venue_qty).abs() > 1e-9 {
                        issues.push(ReconcileIssue::QuantityMismatch {
                            symbol: symbol.clone(),
                            oms_qty,
                            venue_qty,
                        });
                    }
                }
                None => {
                    if oms_qty.abs() > 1e-9 {
                        issues.push(ReconcileIssue::MissingVenuePosition {
                            symbol: symbol.clone(),
                            oms_qty,
                        });
                    }
                }
            }
        }

        // Check for venue positions not tracked by OMS.
        for (symbol, &venue_qty) in venue_positions {
            if !oms_positions.contains_key(symbol) && venue_qty.abs() > 1e-9 {
                issues.push(ReconcileIssue::UntrackedVenuePosition {
                    symbol: symbol.clone(),
                    venue_qty,
                });
            }
        }

        issues
    }

    /// Reconcile: update local state from venue fills.
    pub fn reconcile(&mut self, fills: &[crate::Fill]) {
        for fill in fills {
            // Find the order by id (if we tracked it).
            // For now, just log; full reconciliation in Phase 2.
        }
    }
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export Fill from dococ-core? For now, define locally.
#[derive(Debug, Clone)]
pub struct Fill {
    pub order_id: u64,
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub price: f64,
    pub timestamp: i64,
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
        let id = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, None);
        assert!(mgr.get(id).is_some());
        assert_eq!(mgr.get(id).unwrap().state, OrderState::New);
    }

    #[test]
    fn order_manager_cancel_all() {
        let mut mgr = OrderManager::new();
        let id1 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, None);
        let id2 = mgr.create_order("BTCUSDT", Side::Sell, 0.05, 64_000.0, None);
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

    #[test]
    fn idempotent_key_prevents_duplicate() {
        let mut mgr = OrderManager::new();
        let id1 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, Some("key-1"));
        let id2 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, Some("key-1"));
        assert_eq!(id1, id2);
        assert_eq!(mgr.orders().len(), 1);
    }

    #[test]
    fn different_keys_create_different_orders() {
        let mut mgr = OrderManager::new();
        let id1 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, Some("key-1"));
        let id2 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, Some("key-2"));
        assert_ne!(id1, id2);
        assert_eq!(mgr.orders().len(), 2);
    }

    #[test]
    fn no_key_always_creates_new() {
        let mut mgr = OrderManager::new();
        let id1 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, None);
        let id2 = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, None);
        assert_ne!(id1, id2);
        assert_eq!(mgr.orders().len(), 2);
    }

    #[test]
    fn idempotent_lookup_by_key() {
        let mut mgr = OrderManager::new();
        let id = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, Some("key-1"));
        let found = mgr.find_by_key("key-1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id);
    }

    #[test]
    fn reconcile_finds_quantity_mismatch() {
        let mut mgr = OrderManager::new();
        let id = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, None);
        if let Some(o) = mgr.get_mut(id) {
            o.mark_working();
            o.fill(0.1, 64_032.0);
        }

        let mut venue_positions = std::collections::HashMap::new();
        venue_positions.insert("BTCUSDT".to_string(), 0.05);

        let issues = mgr.reconcile_positions(&venue_positions);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0], ReconcileIssue::QuantityMismatch { .. }));
    }

    #[test]
    fn reconcile_no_issues_when_matching() {
        let mut mgr = OrderManager::new();
        let id = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, None);
        if let Some(o) = mgr.get_mut(id) {
            o.mark_working();
            o.fill(0.1, 64_032.0);
        }

        let mut venue_positions = std::collections::HashMap::new();
        venue_positions.insert("BTCUSDT".to_string(), 0.1);

        let issues = mgr.reconcile_positions(&venue_positions);
        assert!(issues.is_empty());
    }

    #[test]
    fn reconcile_finds_missing_venue_position() {
        let mut mgr = OrderManager::new();
        let id = mgr.create_order("BTCUSDT", Side::Buy, 0.1, 64_000.0, None);
        if let Some(o) = mgr.get_mut(id) {
            o.mark_working();
            o.fill(0.1, 64_032.0);
        }

        let venue_positions = std::collections::HashMap::new();

        let issues = mgr.reconcile_positions(&venue_positions);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0], ReconcileIssue::MissingVenuePosition { .. }));
    }

    #[test]
    fn reconcile_finds_extra_venue_position() {
        let mgr = OrderManager::new();

        let mut venue_positions = std::collections::HashMap::new();
        venue_positions.insert("ETHUSDT".to_string(), 1.0);

        let issues = mgr.reconcile_positions(&venue_positions);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0], ReconcileIssue::UntrackedVenuePosition { .. }));
    }
}
