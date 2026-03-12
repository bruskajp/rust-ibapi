use crate::accounts::Position;
use crate::orders::{Action, Order};
use crate::prelude::Contract;
use polars::prelude::search_sorted::binary_search_ca;
use polars::prelude::*;
use chrono::{DateTime, Utc, TimeZone};
use chrono::offset::LocalResult;
use anyhow::*;

use super::types::Ohlcv;

use super::exchange_2::Exchange2;

pub struct IbBacktestExchange {
    data: DataFrame,
    start_idx: usize,
    end_idx: usize,
    current_idx: usize,
    started: bool,

    open_orders: Vec<(Contract, Order)>,
    completed_orders: Vec<(Contract, Order)>,
    signal_id_counter: u64,

    open_positions: Vec<Position>,
}


impl IbBacktestExchange {
    pub fn new(data: DataFrame, start_ts: Option<DateTime<Utc>>, end_ts: Option<DateTime<Utc>>) -> Result<Self> {
        // Set the dates if not provided, using the first and last timestamps from the DataFrame
        let dates = data.column("datetime")?.datetime()?.physical();
        let first_ts = get_date_from_col(dates, 0)?;
        let last_ts = get_date_from_col(dates, data.height() - 1)?;
        let start_idx = match start_ts {
            Some(ts) => {
                // if ts < first_ts || ts > last_ts {
                //     return Err(anyhow!("Start timestamp {} is out of bounds of the data range {} - {}", ts, first_ts, last_ts));
                // }
                find_next_valid_timestamp_idx(dates, ts)
            },
            None => 0,
        };
        // let start_ts = get_date_from_col(dates, start_idx)?;

        let end_idx = match end_ts {
            Some(ts) => {
                if ts < first_ts || ts > last_ts {
                    return Err(anyhow!("End timestamp {} is out of bounds of the data range {} - {}", ts, first_ts, last_ts));
                }
                find_next_valid_timestamp_idx(dates, ts)
            },
            None => data.height() - 1,
        };
        // let end_ts = get_date_from_col(dates, end_idx)?;

        let current_idx: usize = start_idx;

        // Ok(BacktestExchange { data, start_ts, current_idx, end_ts })
        let exchange = IbBacktestExchange {
            data,
            start_idx,
            end_idx,
            current_idx,
            started: false,
            open_orders: Vec::new(),
            completed_orders: Vec::new(),
            signal_id_counter: 0,
            open_positions: Vec::new(),
        };

        Ok(exchange)
    }

    #[allow(dead_code)]
    pub fn get_data(&self) -> &DataFrame {
        &self.data
    }

    #[allow(dead_code)]
    pub fn get_eval_data(&self) -> DataFrame {
        let offset = self.start_idx.try_into().unwrap();
        self.data.slice(offset, self.end_idx - self.start_idx + 1)
    }

    pub fn get_history(&self) -> DataFrame {
        self.data.slice(0, self.current_idx)
    }
}

fn create_backtest_position(contract: Contract, position: f64, average_cost: f64) -> Position {
    Position {
        account: "backtest".into(),
        contract,
        position,
        average_cost,
    }
}

impl Exchange2 for IbBacktestExchange {
    fn next_value(&mut self) -> Result<Option<Ohlcv>> {
        // Move to the next index
        if self.started {
            self.current_idx += 1;
        } else {
            self.started = true;
        }

        // Check if we've reached the end of the data
        if self.current_idx > self.end_idx {
            return Ok(None);
        }

        // Create the OHLCV data for that index
        let ohlcv = Ohlcv {
            datetime: self.data.column("datetime")?.datetime()?.physical().get(self.current_idx).unwrap(),
            open: self.data.column("open")?.f64()?.get(self.current_idx).unwrap(),
            high: self.data.column("high")?.f64()?.get(self.current_idx).unwrap(),
            low: self.data.column("low")?.f64()?.get(self.current_idx).unwrap(),
            close: self.data.column("close")?.f64()?.get(self.current_idx).unwrap(),
            volume: self.data.column("volume")?.i64()?.get(self.current_idx).unwrap(),
        };

        // Compute which orders to complete
        for (contract, order) in self.open_orders.clone() {
            let current_position = self.open_positions.iter().find(|p| p.contract == contract);
            if current_position.is_none() {
                self.open_positions.push(create_backtest_position(contract.clone(), 0.0, 0.0));
            }
            let current_position = self.open_positions.iter_mut().find(|p| p.contract == contract).unwrap();
            if order.order_type == "MKT" { // Market order
                match order.action {
                    Action::Buy =>
                        current_position.position += order.total_quantity,
                    Action::Sell =>
                        // return Err(anyhow!("Market sell orders are not supported in this backtest implementation because it can lead to unrealistic fills. Please use limit orders with a reasonable limit price instead.")),
                        current_position.position -= order.total_quantity,
                    _ => return Err(anyhow!("Unknown order action: {}", order.action)),
                };
                self.completed_orders.push((contract.clone(), order.clone()));
                self.open_orders.retain(|(_, o)| o.order_id != order.order_id);
                println!("Filled {} market order for {} contracts", order.action, order.total_quantity);
            } else if order.order_type == "STP" { // Stop Loss
                let stop_price = order.aux_price.unwrap();
                match order.action {
                    Action::Buy => {
                        if ohlcv.high >= stop_price {
                            current_position.position += order.total_quantity;
                            self.completed_orders.push((contract.clone(), order.clone()));
                            self.open_orders.retain(|(_, o)| o.order_id != order.order_id);
                            self.open_orders.retain(|(_, o)| o.parent_id != order.parent_id);
                            println!("Filled BUY stop order at price {} (current high {})", stop_price, ohlcv.high);
                        }
                    },
                    Action::Sell => {
                        if ohlcv.low <= stop_price {
                            current_position.position -= order.total_quantity;
                            self.completed_orders.push((contract.clone(), order.clone()));
                            self.open_orders.retain(|(_, o)| o.order_id != order.order_id);
                            self.open_orders.retain(|(_, o)| o.parent_id != order.parent_id);
                            println!("Filled SELL stop order at price {} (current low {})", stop_price, ohlcv.low);
                        }
                    },
                    _ => return Err(anyhow!("Unknown order action: {}", order.action)),
                };
            } else if order.order_type == "LMT" { // Take Profit
                let limit_price = order.limit_price.unwrap();
                match order.action {
                    Action::Buy => {
                        if ohlcv.low <= limit_price {
                            let bracket_orders = self.open_orders.iter().filter(|(_, o)| o.parent_id == order.parent_id).collect::<Vec<_>>();
                            if bracket_orders.len() == 0 {
                                return Err(anyhow!("Limit order {} does not have its corresponding stop loss bracket order. This means that the backtesting algorithm hit the stop loss AND the take profit in the same candlestick. Please use higher resolution data or change your algorithm.", order.order_id));
                            }
                            current_position.position += order.total_quantity;
                            self.completed_orders.push((contract.clone(), order.clone()));
                            self.open_orders.retain(|(_, o)| o.order_id != order.order_id);
                            self.open_orders.retain(|(_, o)| o.parent_id != order.parent_id);
                            println!("Filled BUY limit order at price {} (current low {})", limit_price, ohlcv.low);
                        }
                    },
                    Action::Sell => {
                        if ohlcv.high >= limit_price {
                            let bracket_orders = self.open_orders.iter().filter(|(_, o)| o.parent_id == order.parent_id).collect::<Vec<_>>();
                            if bracket_orders.len() == 0 {
                                return Err(anyhow!("Limit order {} does not have its corresponding stop loss bracket order. This means that the backtesting algorithm hit the stop loss AND the take profit in the same candlestick. Please use higher resolution data or change your algorithm.", order.order_id));
                            }
                            current_position.position -= order.total_quantity;
                            self.completed_orders.push((contract.clone(), order.clone()));
                            self.open_orders.retain(|(_, o)| o.order_id != order.order_id);
                            self.open_orders.retain(|(_, o)| o.parent_id != order.parent_id);
                            println!("Filled SELL limit order at price {} (current high {})", limit_price, ohlcv.high);
                        }
                    },
                    _ => return Err(anyhow!("Unknown order action: {}", order.action)),
                };
            } else {
                return Err(anyhow!("Unknown order type: {}", order.order_type));
            }
        }

        self.open_positions.retain(|p| p.position != 0.0);

        Ok(Some(ohlcv))
    }

    // fn order(&mut self, pos: Position, amt: f64, stop_loss: Option<f64>, take_profit: Option<f64>) -> Result<Signal> {
    fn order(&mut self, order: Order, contract: Contract) -> Result<()> {
        // let ts = self.data.column("datetime")?.datetime()?.physical().get(self.current_idx).unwrap();

        println!("Placed {} order {} of {}", contract.symbol, order.order_id, order.order_type);

        // TODO: (needed) I need to cancel LIMIT orders somehow!!!
        self.open_orders.push((contract, order));
        self.signal_id_counter += 1;

        for order in self.open_orders.iter() {
            println!("Open order: contract={}, order_id={}, type={}, parent_id={}", order.0.symbol, order.1.order_id, order.1.order_type, order.1.parent_id);
        }   
        for pos in self.open_positions.iter() {
            println!("Position: {} {}", pos.contract.symbol, pos.position);
        }
        println!("---");

        Ok(())
    }

    fn cancel_order(&mut self, order_id: i32) -> Result<()> {
        self.open_orders.retain(|(_, o)| o.order_id != order_id);
        Ok(())
    }

    fn get_open_orders(&self) -> &Vec<(Contract, Order)> {
        &self.open_orders
    }

    fn get_completed_orders(&self) -> &Vec<(Contract, Order)> {
        &self.completed_orders
    }

    fn get_open_positions(&self) -> &Vec<Position> {
        &self.open_positions
    }
}

pub fn get_date_from_col(col: &ChunkedArray<Int64Type>, idx: usize) -> Result<DateTime<Utc>> {
    let ts = col.get(idx).unwrap();
    match Utc.timestamp_micros(ts) {
        LocalResult::Single(datetime) => Ok(datetime),
        LocalResult::Ambiguous(datetime1, datetime2) => Err(anyhow!("Ambiguous timestamp: {} could be either {} or {}", ts, datetime1, datetime2)),
        LocalResult::None => Err(anyhow!("Invalid timestamp: {}", ts)),
    }
}

pub fn get_date(data: &DataFrame, idx: usize) -> Result<DateTime<Utc>> {
    let dates = data.column("datetime")?.datetime()?.physical();
    get_date_from_col(dates, idx)
}

pub fn find_next_valid_timestamp_idx(col: &ChunkedArray<Int64Type>, current_ts: DateTime<Utc>) -> usize {
    let cutoff = current_ts.timestamp_micros();
    // col.into_no_null_iter().position(|v| v >= cutoff).unwrap_or(ts.len());
    binary_search_ca(
            col,
            std::iter::once(Some(cutoff)),
            SearchSortedSide::Left, // "left" insertion point
            false,                  // descending = false (ascending)
        )[0] as usize
}
