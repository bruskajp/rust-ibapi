// -------------------------

use std::{ops::Deref, sync::Arc};

use crate::{
    client::sync::Client, connection::ConnectionMetadata, errors::Error, orders::{self, BracketOrderBuilder, BracketOrderIds, Order, OrderBuilder, OrderId}, prelude::Contract, transport::MessageBus
};

/// Extended Client. Manages the whether the sync client is used for backtesting or for live.
/// Tracks some global information such as server version and server time.
/// Supports generation of order ids.
pub struct ExtendedClient(pub Client);

impl ExtendedClient {
    /// Start building an order for the given contract
    ///
    /// This is the primary API for creating orders, providing a fluent interface
    /// that guides you through the order creation process.
    ///
    /// # Example
    /// ```no_run
    /// use ibapi::client::blocking::Client;
    /// use ibapi::contracts::Contract;
    ///
    /// let client: ExtendedClient = Client::connect("127.0.0.1:4002", 100).expect("connection failed").into();
    /// let contract = Contract::stock("AAPL").build();
    ///
    /// let order_id = client.order(&contract)
    ///     .buy(100)
    ///     .limit(50.0)
    ///     .submit(true).expect("order submission failed");
    /// ```
    pub fn order<'a>(&'a self, contract: &'a Contract) -> OrderBuilder<'a, ExtendedClient> {
        OrderBuilder::new(self, contract)
    }

    // TODO: (needed) Fix submit_oca_orders
    /// Submit multiple OCA (One-Cancels-All) orders
    ///
    /// When one order in the group is filled, all others are automatically cancelled.
    ///
    /// # Example
    /// ```no_run
    /// use ibapi::client::blocking::Client;
    /// use ibapi::contracts::Contract;
    ///
    /// let client = Client::connect("127.0.0.1:4002", 100).expect("connection failed");
    ///
    /// let contract1 = Contract::stock("AAPL").build();
    /// let contract2 = Contract::stock("MSFT").build();
    ///
    /// let order1 = client.order(&contract1)
    ///     .buy(100)
    ///     .limit(50.0)
    ///     .oca_group("MyOCA", 1)
    ///     .build_order().expect("order build failed");
    ///     
    /// let order2 = client.order(&contract2)
    ///     .buy(100)
    ///     .limit(45.0)
    ///     .oca_group("MyOCA", 1)
    ///     .build_order().expect("order build failed");
    ///
    /// let order_ids = client.submit_oca_orders(
    ///     vec![(contract1, order1), (contract2, order2)]
    /// ).expect("OCA submission failed");
    /// ```
    pub fn submit_oca_orders(&self, orders: Vec<(Contract, crate::orders::Order)>) -> Result<Vec<OrderId>, Error> {
        let mut order_ids = Vec::new();

        for (contract, mut order) in orders.into_iter() {
            let order_id = self.next_order_id();
            order.order_id = order_id;
            order_ids.push(OrderId::new(order_id));
            println!("Backtest submit OCA order: contract={:?}, order_id={}, order={:?}", contract, order_id, order);
        }

        Ok(order_ids)
    }
}

impl Deref for ExtendedClient {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Client> for ExtendedClient {
    fn from(c: Client) -> Self {
        Self(c)
    }
}

impl<'a> OrderBuilder<'a, ExtendedClient> {
    /// Submit the order in backtest mode (does not send to IB, simulates fill)
    /// Returns the order ID assigned to the submitted order
    pub fn submit(self, backtest: bool) -> Result<(OrderId, Contract, Order), Error> {
        let client = self.client;
        let contract = self.contract;
        let order_id = client.next_order_id();
        let order = self.build()?;

        if backtest {
            println!("Backtest submit order: contract={:?}, order_id={}, order={:?}", contract, order_id, order);
        } else {
            orders::blocking::submit_order(client, order_id, contract, &order)?;
        }

        println!("Backtest submit order: contract={:?}, order_id={}, order={:?}", contract, order_id, order);

        Ok((OrderId::new(order_id), contract.clone(), order))
    }

    /// Build the order and return it without submitting
    pub fn build_order(self) -> Result<crate::orders::Order, Error> {
        self.build().map_err(Into::into)
    }

    /// Analyze order for margin/commission (what-if)
    pub fn analyze(mut self) -> Result<crate::orders::OrderState, Error> {
        self.what_if = true;
        let client = self.client;
        let contract = self.contract;
        let order_id = client.next_order_id();
        let order = self.build()?;

        // Simulate what-if analysis for backtesting
        // For now, return a default OrderState or mock values
        log::info!(
            "Backtest analyze: contract={:?}, order_id={}, order={:?}",
            contract,
            order_id,
            order
        );

        // You can customize this to return realistic margin/commission estimates
        Ok(crate::orders::OrderState::default())
    }
}

impl<'a> BracketOrderBuilder<'a, ExtendedClient> {
    /// Submit bracket orders in backtest mode (does not send to IB, simulates fill)
    /// Returns BracketOrderIds containing all three order IDs
    pub fn submit_all(self, backtest: bool) -> Result<BracketOrderIds, Error> {
        let client = self.parent_builder.client;
        let contract = self.parent_builder.contract;
        let orders = self.build()?;

        // Reserve all order IDs upfront to prevent collisions
        let parent_id = client.next_order_id();
        let tp_id = client.next_order_id();
        let sl_id = client.next_order_id();
        let reserved_ids = [parent_id, tp_id, sl_id];

        for (i, mut order) in orders.into_iter().enumerate() {
            let order_id = reserved_ids[i];
            order.order_id = order_id;

            // Update parent_id for child orders
            if i > 0 {
                order.parent_id = parent_id;
            }

            // Only transmit the last order
            if i == 2 {
                order.transmit = true;
            }

            if backtest {
                println!(
                    "Backtest submit bracket order: contract={:?}, order_id={}, parent_id={:?}, order={:?}",
                    contract,
                    order_id,
                    if i == 0 { None } else { Some(parent_id) },
                    order
                );
            } else {
                orders::blocking::submit_order(client, order_id, contract, &order)?;
            }
        }

        Ok(BracketOrderIds::new(parent_id, tp_id, sl_id))
    }
}