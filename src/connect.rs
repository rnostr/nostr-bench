use futures_util::{SinkExt, StreamExt, TryStreamExt};
use parking_lot::Mutex;
use std::cmp;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{
    net::{TcpSocket, TcpStream},
    time,
    time::Duration,
};
use tokio_tungstenite::{client_async, tungstenite::Error as WsError, WebSocketStream};
use url::Url;

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

fn parse_interface(s: &str) -> Result<SocketAddr, String> {
    Ok(format!("{}:0", s).parse().map_err(|_| "error format")?)
}

/// Bech time result
#[derive(Default, Debug, Copy, Clone)]
pub struct TimeResult {
    pub count: usize,
    pub total: Duration,
    pub avg: Duration,
    pub min: Duration,
    pub max: Duration,
}

impl TimeResult {
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

#[derive(Default, Debug, Copy, Clone)]
pub struct ConnectResult {
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
    /// Lost connection by some error
    pub lost: usize,
    /// num of closed when alive timeout
    pub close: usize,
    /// success connect times
    pub connect_time: TimeResult,
}

macro_rules! add1 {
    ($name:ident, $attr:ident) => {{
        let mut r = $name.lock();
        r.$attr += 1;
    }};
}

macro_rules! subtract1 {
    ( $name:ident, $attr:ident) => {{
        let mut r = $name.lock();
        r.$attr -= 1;
    }};
}

/// Start bench
pub async fn start(opts: ConnectOpts) {
    let connaddr = Some(parse_wsaddr(&opts.url).unwrap());
    println!("{:?}", opts);
    let result = Arc::new(Mutex::new(ConnectResult {
        total: opts.count,
        ..Default::default()
    }));

    let c_result = result.clone();
    tokio::spawn(async move {
        let interfaces = opts.interface.unwrap_or_default();
        let len = interfaces.len();
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
                match res {
                    Ok(stream) => {
                        {
                            let mut r = result.lock();
                            r.alive += 1;
                            r.connect_time = r.connect_time.add(now.elapsed());
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
            println!("elapsed: {}ms {:?}", now.elapsed().as_millis(), r);
            if r.complete == r.total {
                break;
            }
        }
        time::sleep(Duration::from_secs(2)).await;
    }
}

fn parse_wsaddr(url: &Url) -> std::io::Result<SocketAddr> {
    let addrs = url.socket_addrs(|| match url.scheme() {
        "wss" => Some(443),
        "ws" => Some(80),
        _ => None,
    })?;
    Ok(addrs[0])
}

/// Connect websocket server
pub async fn connect(
    url: Url,
    interface: Option<SocketAddr>,
    connaddr: Option<SocketAddr>,
) -> Result<WebSocketStream<TcpStream>, Error> {
    let connaddr = match connaddr {
        Some(addr) => addr,
        None => parse_wsaddr(&url)?,
    };

    let socket = TcpSocket::new_v4()?;
    if let Some(addr) = interface {
        socket.bind(addr)?;
    }

    let tcp = socket.connect(connaddr).await?;

    let (stream, _) = time::timeout(Duration::from_secs(60), client_async(url, tcp))
        .await
        .map_err(|_| Error::ConnectTimeout)??;
    Ok(stream)
}

/// Wait websocket finish
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
