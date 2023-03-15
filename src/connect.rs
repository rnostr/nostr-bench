use crate::util::{connect, parse_interface, parse_wsaddr, Error};
use crate::{add1, subtract1};
use clap::Parser;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::cmp;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::{time, time::Duration};
use tokio_tungstenite::WebSocketStream;
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
    #[arg(short = 'k', long, default_value = "0", value_name = "NUM")]
    pub keepalive: u64,

    /// Set the amount of threads, default 0 will use all system available cores
    #[arg(short = 't', long, default_value = "0", value_name = "NUM")]
    pub threads: usize,

    /// Network interface address list
    #[arg(short = 'i', long, value_name = "IP", value_parser = parse_interface)]
    pub interface: Option<Vec<SocketAddr>>,
}

/// Bech time result
#[serde_as]
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

/// Start bench
pub async fn start(opts: ConnectOpts) {
    let connaddr = Some(parse_wsaddr(&opts.url).unwrap());
    let stats = Arc::new(Mutex::new(ConnectStats {
        total: opts.count,
        ..Default::default()
    }));

    let c_stats = stats.clone();
    tokio::spawn(async move {
        let interfaces = opts.interface.unwrap_or_default();
        let len = interfaces.len();
        let start_time = time::Instant::now();
        for i in 0..opts.count {
            let url = opts.url.clone();
            let stats = c_stats.clone();
            let interface = if len > 0 {
                Some(interfaces[i % len])
            } else {
                None
            };
            tokio::spawn(async move {
                add1!(stats, connect);
                let now = time::Instant::now();
                let res = connect(url, interface, connaddr).await;
                {
                    let mut r = stats.lock();
                    r.time = start_time.elapsed();
                }
                match res {
                    Ok(stream) => {
                        {
                            let mut r = stats.lock();
                            r.alive += 1;
                            r.success_time = r.success_time.add(now.elapsed());
                        }
                        let res = wait(stream, opts.keepalive).await;
                        subtract1!(stats, alive);
                        if let Err(Error::AliveTimeout) = res {
                            add1!(stats, close);
                        } else {
                            add1!(stats, lost);
                        }
                    }
                    Err(_err) => {
                        // println!("error {:?}", _err);
                        add1!(stats, error);
                    }
                }
                add1!(stats, complete);
            });
            if (i + 1) % opts.rate == 0 {
                time::sleep(Duration::from_secs(1)).await;
            }
        }
    });

    let now = time::Instant::now();
    loop {
        {
            let r = stats.lock();
            // println!(
            //     "elapsed: {}ms {}",
            //     now.elapsed().as_millis(),
            //     serde_json::to_value(r.deref()).unwrap(),
            // );
            println!("elapsed: {}ms {:?}", now.elapsed().as_millis(), r);
            if r.complete == r.total {
                break;
            }
        }
        time::sleep(Duration::from_secs(2)).await;
    }
}

/// Wait websocket close
pub async fn wait(stream: WebSocketStream<TcpStream>, keepalive: u64) -> Result<(), Error> {
    let (mut write, read) = stream.split();
    let stay = read.try_for_each(|_message| async { Ok(()) });

    let result = if keepalive == 0 {
        Ok(stay.await)
    } else {
        time::timeout(Duration::from_secs(keepalive), stay)
            .await
            .map_err(|_| Error::AliveTimeout)
    };
    if let Err(_) = result {
        write.close().await.map_err(|_| Error::AliveTimeout)?;
    }
    result?.map_err(|_| Error::Lost)?;
    Ok(())
}
