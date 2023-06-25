use crate::util::{gen_string, parse_interface};
use crate::{add1, bench_message, BenchOpts, Error, MessageStats};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio_tungstenite::MaybeTlsStream;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::{time, time::Duration};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use url::Url;

/// Echo benchmark options
#[derive(Debug, Clone, Parser)]
pub struct EchoOpts {
    /// Nostr relay host url
    #[arg(value_name = "URL")]
    pub url: Url,

    /// Count of clients
    #[arg(short = 'c', long, default_value = "100", value_name = "NUM")]
    pub count: usize,

    /// Open connection rate every second
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

    /// Display stats information as json, time format as milli seconds
    #[arg(long)]
    pub json: bool,

    /// The message bytes size for send/receive
    #[arg(long, default_value = "512", value_name = "NUM")]
    pub size: usize,
}

/// Start bench
pub async fn start(opts: EchoOpts) {
    let bench_opts = BenchOpts {
        url: opts.url,
        count: opts.count,
        rate: opts.rate,
        keepalive: opts.keepalive,
        threads: opts.threads,
        interface: opts.interface,
    };

    let stats = Arc::new(Mutex::new(MessageStats {
        total: 0,
        ..Default::default()
    }));

    let c_stats = stats.clone();
    let message = gen_string(opts.size);

    bench_message(bench_opts, stats, opts.json, |stream| {
        loop_message(stream, c_stats, message)
    })
    .await;
}

/// Loop send message
async fn loop_message(
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    stats: Arc<Mutex<MessageStats>>,
    message: String,
) -> Result<(), Error> {
    let (mut write, mut read) = stream.split();
    time::sleep(Duration::from_secs(1)).await;
    let mut start = time::Instant::now();
    add1!(stats, total);
    write.send(Message::Text(message.clone())).await?;
    loop {
        let msg = read.next().await;
        let message = message.clone();
        match msg {
            Some(msg) => {
                let msg = msg?;
                if msg.is_text() || msg.is_binary() {
                    {
                        let mut r = stats.lock();
                        r.size += msg.len() + message.len();
                    }
                    {
                        let mut r = stats.lock();
                        r.success_time = r.success_time.add(start.elapsed());
                    }
                    add1!(stats, complete, total);
                    // let event = "test".to_string();
                    start = time::Instant::now();
                    write.send(Message::Text(message)).await?;
                } else if msg.is_close() {
                    break;
                }
            }
            None => break,
        }
    }
    Ok(())
}
