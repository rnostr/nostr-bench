use crate::util::{gen_note_event, parse_interface};
use crate::{add1, bench, BenchOpts, Error, TimeStats};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::ops::Deref;
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

/// Bench event stats
#[derive(Default, Debug, Copy, Clone, Deserialize, Serialize)]
pub struct EventStats {
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

    let event_stats = Arc::new(Mutex::new(EventStats {
        total: 0,
        ..Default::default()
    }));

    let c_stats = event_stats.clone();

    bench_event(bench_opts, event_stats, opts.json, |stream| {
        loop_event(stream, c_stats)
    })
    .await;
}

pub(crate) async fn bench_event<F, Fut>(
    opts: BenchOpts,
    event_stats: Arc<Mutex<EventStats>>,
    json: bool,
    handler: F,
) where
    F: FnOnce(WebSocketStream<TcpStream>) -> Fut + Send + Sync + Clone + 'static,
    Fut: core::future::Future<Output = Result<(), Error>> + Send + 'static,
{
    let mut last_count: usize = 0;
    let mut last_size: usize = 0;
    let mut last_time = time::Instant::now();
    bench(opts, handler, move |now, stats| {
        let st = event_stats.lock();
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
                "connect_stats": stats,
                "event_stats": st.deref(),
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
                "tps: {}/s transfer: {}MB/s complate: {} error: {} time: [{}]",
                tps, size, st.complete, st.error, time,
            );
            println!(
                "elapsed: {}ms connections: {} message {}",
                now.elapsed().as_millis(),
                stats.alive,
                message,
            );
        }
        last_count = st.complete - st.error;
        last_size = st.size;
        last_time = time::Instant::now();
    })
    .await;
}

/// Loop send event
pub async fn loop_event(
    stream: WebSocketStream<TcpStream>,
    stats: Arc<Mutex<EventStats>>,
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
                    add1!(stats, complete, total);
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
