use anyhow::*;
use crate::{accounts::Position, orders::Order, prelude::Contract};

use super::types::*;


/// Represents a financial exchange interface for trading operations.
pub trait Exchange2 {
    /// Called when a new value is available from the exchange.
    fn next_value(&mut self) -> Result<Option<Ohlcv>>;

    /// Opens an order with the specified value.
    ///
    /// # Arguments
    ///
    /// * `order` - The order details including action, quantity, price, etc.
    /// * `contract` - The financial contract associated with the order.
    ///
    /// # Returns
    ///
    /// Returns a unique identifier for the opened order.
    ///
    /// # Example
    ///
    /// ```
    /// let position_id = exchange.order(order, contract);
    /// ```
    fn order(&mut self, order: Order, contract: Contract) -> Result<()>;

    /// Cancels an existing order with the specified identifier.
    ///
    /// # Arguments
    ///
    ///  * `order_id` - The unique identifier of the order to cancel.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the order was successfully canceled, or an error if the cancellation failed.
    /// 
    /// # Example
    ///
    /// ```
    /// exchange.cancel_order(0)?;
    /// ```
    fn cancel_order(&mut self, order_id: i32) -> Result<()>;

    fn get_open_orders(&self) -> &Vec<(Contract, Order)>;

    fn get_completed_orders(&self) -> &Vec<(Contract, Order)>;

    fn get_open_positions(&self) -> &Vec<Position>;
}
