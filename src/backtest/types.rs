#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ohlcv {
    pub datetime: i64, // Unix timestamp in microseconds
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

impl Ohlcv {
    pub fn new(datetime: i64, open: f64, high: f64, low: f64, close: f64, volume: i64) -> Self {
        Ohlcv {
            datetime,
            open,
            high,
            low,
            close,
            volume,
        }
    }

    pub fn new_zero() -> Self {
        Ohlcv {
            datetime: 0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Position {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signal {
    pub id: u64,
    pub position: Position,
    pub amount: f64,
    pub timestamp: i64,
    pub entry_value: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompletedSignal {
    pub id: u64,
    pub position: Position,
    pub amount: f64,
    pub entry_timestamp: i64,
    pub entry_value: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub exit_timestamp: i64,
    pub exit_value: f64,
    pub profit: f64,
}

impl CompletedSignal {
    pub fn new_from_signal(signal: Signal, exit_timestamp: i64, exit_value: f64) -> Self {
        let profit = if signal.position == Position::Long {
            (exit_value - signal.entry_value) * signal.amount
        } else {
            (signal.entry_value - exit_value) * signal.amount
        };

        CompletedSignal {
            id: signal.id,
            position: signal.position,
            amount: signal.amount,
            entry_timestamp: signal.timestamp,
            entry_value: signal.entry_value,
            stop_loss: signal.stop_loss,
            take_profit: signal.take_profit,
            exit_timestamp,
            exit_value,
            profit,
        }
    }
}
