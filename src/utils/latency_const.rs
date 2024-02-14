pub const LATENCY_CHANNEL_LABEL: &str = "latency";
// address of the NTP server, more info in: https://www.ntppool.org/es/
pub const SNTP_POOL_ADDR: &str = "pool.ntp.org:123";
pub const UDP_SOCKET_ADDR: &str = "0.0.0.0:0";
pub const UDP_SOCKET_TIMEOUT: u64 = 2;
pub const LOOP_LATENCY_TIME: u64 = 2;
pub const MAX_SNTP_RETRY: u8 = 3;
pub const SNTP_SEND_SLEEP: u64 = 500;
