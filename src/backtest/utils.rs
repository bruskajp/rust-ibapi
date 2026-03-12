use chrono::{DateTime, TimeZone, Utc, offset::LocalResult};
use anyhow::*;

pub fn unix_us_to_datetime(unix_ts: i64) -> Result<DateTime<Utc>> {
    // DateTime::from_timestamp(unix_ts / 1_000_000, ((unix_ts % 1_000_000) * 1000) as u32)
    //     .ok_or_else(|| anyhow!("Invalid unix microsecond timestamp: {}", unix_ts));

    match Utc.timestamp_micros(unix_ts) {
        LocalResult::Single(datetime) => Ok(datetime),
        LocalResult::Ambiguous(datetime1, datetime2) => Err(anyhow!("Ambiguous timestamp: {} could be either {} or {}", unix_ts, datetime1, datetime2)),
        LocalResult::None => Err(anyhow!("Invalid timestamp: {}", unix_ts)),
    }
}