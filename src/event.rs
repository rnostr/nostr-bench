use crate::util::{gen_note_event, parse_interface};
use crate::{add1, bench, BenchOpts, Error, TimeStats};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
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
}

/// Start bench
pub async fn start(opts: EventOpts) {
    let opts = BenchOpts {
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
    let mut last: usize = 0;
    let mut last_time = time::Instant::now();
    let c_stats = event_stats.clone();
    bench(
        opts,
        |stream| loop_message(stream, c_stats),
        |now, r| {
            let event_s = event_stats.lock();
            let cur = event_s.complete - event_s.error - last;
            let tps = if last_time.elapsed().as_secs() > 1 {
                cur as f64 / last_time.elapsed().as_secs_f64()
            } else {
                0.0
            };

            // println!(
            //     "elapsed: {}ms {}, {}",
            //     now.elapsed().as_millis(),
            //     serde_json::to_string(s.deref()).unwrap(),
            //     serde_json::to_string(event_s.deref()).unwrap(),
            // );
            println!(
                "elapsed: {}ms tps: {}/s {:?}, {:?}",
                now.elapsed().as_millis(),
                tps as u64,
                r,
                event_s,
            );
            last = event_s.complete - event_s.error;
            last_time = time::Instant::now();
        },
    )
    .await;
}

/// Loop sent event
async fn loop_message(
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
                    let msg = msg.to_string();
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
                    let event = gen_note_event(BENCH_CONTENT);
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
