use std::time::Duration;

pub const UDP_BUF_SIZE: usize = 1432;

pub const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(1);
pub const KEEP_ALIVE_MISSED_DROP_CONNECTION: Duration = Duration::from_secs(5);
