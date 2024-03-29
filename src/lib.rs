use futures_util::{
    future::{join_all, select},
    Future,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::cmp;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use tokio::{
    net::{TcpSocket, TcpStream},
    time,
    time::Duration,
};
use tokio_tungstenite::{
    client_async_tls, tungstenite::Error as WsError, MaybeTlsStream, WebSocketStream,
};
use url::Url;
use util::parse_wsaddr;

pub mod connect;
pub mod echo;
pub mod event;
pub mod req;
pub mod runtime;
pub mod util;

#[macro_export]
macro_rules! add1 {
    ($name:ident, $($attr:ident) , *) => {{
        let mut r = $name.lock();
        $(
            r.$attr += 1;
        )*
    }};
}

#[macro_export]
macro_rules! subtract1 {
    ($name:ident, $($attr:ident) , *) => {{
        let mut r = $name.lock();
        $(
            r.$attr -= 1;
        )*
    }};
}

/// Connection error
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// I/O error
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),
    /// Ws error
    #[error("ws error: {0}")]
    Ws(#[from] WsError),
    /// Connect timeout
    #[error("connect timeout")]
    ConnectTimeout,
    /// Alive timeout
    #[error("alive timeout")]
    AliveTimeout,
    /// Lost connection
    #[error("lost connection")]
    Lost,
}

/// Connection options
#[derive(Debug, Clone)]
pub struct BenchOpts {
    /// Nostr relay host url
    pub url: Url,

    /// Max count of clients
    pub count: usize,

    /// Start open connection rate every second
    pub rate: usize,

    /// Close connection after second, ignore when set to 0
    pub keepalive: u64,

    /// Set the amount of threads, default 0 will use all system available cores
    pub threads: usize,

    /// Network interface address list
    pub interface: Option<Vec<SocketAddr>>,
}

/// Time stats
#[serde_as]
#[derive(Default, Debug, Copy, Clone, Deserialize, Serialize)]
pub struct TimeStats {
    pub count: usize,
    /// total time, milli seconds in json format
    #[serde_as(as = "DurationMilliSeconds")]
    pub total: Duration,
    #[serde_as(as = "DurationMilliSeconds")]
    pub avg: Duration,
    #[serde_as(as = "DurationMilliSeconds")]
    pub min: Duration,
    #[serde_as(as = "DurationMilliSeconds")]
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

/// Connect stats
#[serde_as]
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
    #[serde_as(as = "DurationMilliSeconds")]
    pub time: Duration,
    /// success connect times result
    pub success_time: TimeStats,
}

/// Message stats
#[derive(Default, Debug, Copy, Clone, Deserialize, Serialize)]
pub struct MessageStats {
    /// total event send
    pub total: usize,
    /// num of completed
    pub complete: usize,
    /// num of connect error
    pub error: usize,
    /// success event times stats
    pub success_time: TimeStats,
    /// message transfer size as bytes
    pub size: usize,
    /// total event received
    pub event: usize,
}

/// Start bench
pub async fn bench<F, Fut, P>(opts: BenchOpts, handler: F, mut printer: P)
where
    F: FnOnce(WebSocketStream<MaybeTlsStream<TcpStream>>) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
    P: FnMut(time::Instant, &ConnectStats) + Send + 'static,
{
    let connaddr = Some(parse_wsaddr(&opts.url).unwrap());
    let stats = Arc::new(Mutex::new(ConnectStats {
        total: opts.count,
        ..Default::default()
    }));
    let c_stats = stats.clone();

    let run_print = tokio::spawn(async move {
        let now = time::Instant::now();
        loop {
            {
                let r = stats.lock();
                printer(now, r.deref());
                if r.complete == r.total {
                    break;
                }
            }
            time::sleep(Duration::from_secs(2)).await;
        }
    });

    let run_connect = tokio::spawn(async move {
        let interfaces = opts.interface.unwrap_or_default();
        let len = interfaces.len();
        let start_time = time::Instant::now();
        let mut tasks = vec![];
        for i in 0..opts.count {
            let url = opts.url.clone();
            let stats = c_stats.clone();
            let interface = if len > 0 {
                Some(interfaces[i % len])
            } else {
                None
            };
            let handler = handler.clone();
            let task = tokio::spawn(async move {
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

                        let res = keepalive(opts.keepalive, handler(stream)).await;
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
            tasks.push(task);
            if (i + 1) % opts.rate == 0 {
                time::sleep(Duration::from_secs(1)).await;
            }
        }
        join_all(tasks).await;
    });
    select(run_print, run_connect).await;
}

/// Start bench with message stats
pub async fn bench_message<F, Fut>(
    opts: BenchOpts,
    stats: Arc<Mutex<MessageStats>>,
    json: bool,
    handler: F,
) where
    F: FnOnce(WebSocketStream<MaybeTlsStream<TcpStream>>) -> Fut + Send + Sync + Clone + 'static,
    Fut: core::future::Future<Output = Result<(), Error>> + Send + 'static,
{
    let mut last_count: usize = 0;
    let mut last_size: usize = 0;
    let mut last_time = time::Instant::now();
    bench(opts, handler, move |now, cstats| {
        let st = stats.lock();
        let cur_count = st.complete - st.error - last_count;
        let tps = if last_time.elapsed().as_secs() > 1 {
            cur_count as f64 / last_time.elapsed().as_secs_f64()
        } else {
            0.0
        };

        let cur_size = st.size - last_size;
        let size = if last_time.elapsed().as_secs() > 1 {
            cur_size as f64 / last_time.elapsed().as_secs_f64()
        } else {
            0.0
        };
        let tps = tps as u64;
        let size = (((size / 100000.0) as u64) as f64) / 10.0;

        if json {
            let json = serde_json::json!({
                "elapsed": now.elapsed().as_millis(),
                "last_elapsed": last_time.elapsed().as_millis(),
                "tps": tps,
                "size": size,
                "connect_stats": cstats,
                "message_stats": st.deref(),
            });
            println!("{}", serde_json::to_string(&json).unwrap());
        } else {
            let time = st.success_time;
            let time = format!(
                "avg: {}ms max: {}ms min: {}ms",
                time.avg.as_millis(),
                time.max.as_millis(),
                time.min.as_millis(),
            );
            let message = format!(
                "tps: {}/s transfer: {}MB/s complate: {} event: {} error: {} time: [{}]",
                tps, size, st.complete, st.event, st.error, time,
            );
            println!(
                "elapsed: {}ms connections: {} message {}",
                now.elapsed().as_millis(),
                cstats.alive,
                message,
            );
        }
        last_count = st.complete - st.error;
        last_size = st.size;
        last_time = time::Instant::now();
    })
    .await;
}

/// Connect websocket server with bind interface address
pub async fn connect(
    url: Url,
    interface: Option<SocketAddr>,
    connaddr: Option<SocketAddr>,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Error> {
    let connaddr = match connaddr {
        Some(addr) => addr,
        None => parse_wsaddr(&url)?,
    };

    let socket = TcpSocket::new_v4()?;
    if let Some(addr) = interface {
        socket.bind(addr)?;
    }
    let tcp = socket.connect(connaddr).await?;

    let (stream, _) = time::timeout(Duration::from_secs(60), client_async_tls(url, tcp))
        .await
        .map_err(|_| Error::ConnectTimeout)??;
    Ok(stream)
}

/// Return AliveTimeout error when timeout
pub async fn keepalive<T: Future<Output = Result<(), Error>>>(
    second: u64,
    stay: T,
) -> Result<(), Error> {
    let result = if second == 0 {
        Ok(stay.await)
    } else {
        time::timeout(Duration::from_secs(second), stay)
            .await
            .map_err(|_| Error::AliveTimeout)
    };
    result?.map_err(|_| Error::Lost)?;
    Ok(())
}
