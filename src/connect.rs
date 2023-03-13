use crate::util::{connect, parse_interface, parse_wsaddr, wait, Error};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use tokio::{time, time::Duration};
use url::Url;

/// Connection benchmark options
#[derive(Debug, Clone, Parser)]
pub struct ConnectOpts {
    /// Nostr relay host url
    #[arg(value_name = "URL")]
    pub url: Url,

    /// Max count of clients
    #[arg(short = 'c', long, default_value = "100", value_name = "NUM")]
    pub count: usize,

    /// Start open connection rate every second
    #[arg(short = 'r', long, default_value = "50", value_name = "NUM")]
    pub rate: usize,

    /// Close connection after second, ignore when set to 0
    #[arg(short = 'k', long, default_value = "600", value_name = "NUM")]
    pub keepalive: u64,

    /// Set the amount of threads
    #[arg(short = 't', long, default_value = "1", value_name = "NUM")]
    pub threads: usize,

    /// Network interface address list
    #[arg(short = 'i', long, value_name = "IP", value_parser = parse_interface)]
    pub interface: Option<Vec<SocketAddr>>,
}

/// Bech time result
#[derive(Default, Debug, Copy, Clone, Deserialize, Serialize)]
pub struct TimeStats {
    pub count: usize,
    pub total: Duration,
    pub avg: Duration,
    pub min: Duration,
    pub max: Duration,
}

impl TimeStats {
    pub fn add(self, time: Duration) -> Self {
        let total = self.total + time;
        let min = if self.min.is_zero() {
            time
        } else {
            cmp::min(self.min, time)
        };
        Self {
            count: self.count + 1,
            total,
            avg: total / (self.count + 1).try_into().unwrap(),
            min,
            max: cmp::max(time, self.max),
        }
    }
}

/// Bench result

#[derive(Default, Debug, Copy, Clone, Deserialize, Serialize)]
pub struct ConnectStats {
    /// total
    pub total: usize,
    /// num of completed
    pub complete: usize,
    /// num of has connected
    pub connect: usize,
    /// num of connecting
    pub alive: usize,
    /// num of connect error
    pub error: usize,
    /// num of lost connection by some error
    pub lost: usize,
    /// num of closed when alive timeout
    pub close: usize,
    /// time duration when connected
    pub time: Duration,
    /// success connect times result
    pub success_time: TimeStats,
}

macro_rules! add1 {
    ($name:ident, $($attr:ident) , *) => {{
        let mut r = $name.lock();
        $(
            r.$attr += 1;
        )*
    }};
}

macro_rules! subtract1 {
    ($name:ident, $($attr:ident) , *) => {{
        let mut r = $name.lock();
        $(
            r.$attr -= 1;
        )*
    }};
}

/// Start bench
pub async fn start(opts: ConnectOpts) {
    let connaddr = Some(parse_wsaddr(&opts.url).unwrap());
    println!("{:?}", opts);
    let result = Arc::new(Mutex::new(ConnectStats {
        total: opts.count,
        ..Default::default()
    }));

    let c_result = result.clone();
    tokio::spawn(async move {
        let interfaces = opts.interface.unwrap_or_default();
        let len = interfaces.len();
        let start_time = time::Instant::now();
        for i in 0..opts.count {
            let url = opts.url.clone();
            let result = c_result.clone();
            let interface = if len > 0 {
                Some(interfaces[i % len])
            } else {
                None
            };
            tokio::spawn(async move {
                add1!(result, connect);
                let now = time::Instant::now();
                let res = connect(url, interface, connaddr).await;
                {
                    let mut r = result.lock();
                    r.time = start_time.elapsed();
                }
                match res {
                    Ok(stream) => {
                        {
                            let mut r = result.lock();
                            r.alive += 1;
                            r.success_time = r.success_time.add(now.elapsed());
                        }
                        let res = wait(stream, opts.keepalive).await;
                        subtract1!(result, alive);
                        if let Err(Error::AliveTimeout) = res {
                            add1!(result, close);
                        } else {
                            add1!(result, lost);
                        }
                    }
                    Err(_err) => {
                        // println!("error {:?}", _err);
                        add1!(result, error);
                    }
                }
                add1!(result, complete);
            });
            if (i + 1) % opts.rate == 0 {
                time::sleep(Duration::from_secs(1)).await;
            }
        }
    });

    let now = time::Instant::now();
    loop {
        {
            let r = result.lock();
            println!(
                "elapsed: {}ms {}",
                now.elapsed().as_millis(),
                serde_json::to_string(r.deref()).unwrap()
            );
            if r.complete == r.total {
                break;
            }
        }
        time::sleep(Duration::from_secs(2)).await;
    }
}
