use crate::util::{gen_note_event, parse_interface};
use crate::{add1, bench_message, BenchOpts, Error, MessageStats};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::{time, time::Duration};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use url::Url;
const BENCH_CONTENT: &str = "This is a message from nostr-bench client";

/// Event benchmark options
#[derive(Debug, Clone, Parser)]
pub struct EventOpts {
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
}

/// Start bench
pub async fn start(opts: EventOpts) {
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

    bench_message(bench_opts, stats, opts.json, |stream| {
        loop_event(stream, c_stats)
    })
    .await;
}

/// Loop send event
pub async fn loop_event(
    stream: WebSocketStream<TcpStream>,
    stats: Arc<Mutex<MessageStats>>,
) -> Result<(), Error> {
    let (mut write, mut read) = stream.split();
    let event = gen_note_event(BENCH_CONTENT);
    time::sleep(Duration::from_secs(1)).await;
    let mut start = time::Instant::now();
    add1!(stats, total);
    write.send(Message::Text(event)).await?;
    loop {
        let msg = read.next().await;
        match msg {
            Some(msg) => {
                let msg = msg?;
                if msg.is_text() {
                    let event = gen_note_event(BENCH_CONTENT);
                    let msg = msg.to_string();
                    {
                        let mut r = stats.lock();
                        r.size += msg.len() + event.len();
                    }
                    if msg.contains("OK") && msg.contains("true") {
                        {
                            let mut r = stats.lock();
                            r.success_time = r.success_time.add(start.elapsed());
                        }
                    } else {
                        // println!("message error {:?}", msg);
                        add1!(stats, error);
                    }
                    add1!(stats, complete, total, event);
                    // let event = "test".to_string();
                    start = time::Instant::now();
                    write.send(Message::Text(event)).await?;
                } else if msg.is_close() {
                    break;
                }
            }
            None => break,
        }
    }
    Ok(())
}
