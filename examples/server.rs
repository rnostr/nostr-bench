//! A simple async server.
//!
//! You can test this out by running:
//!
//!     cargo run --example server 127.0.0.1:12345
//!

use std::env;

use flume::{unbounded, Sender};
use futures_util::{SinkExt, StreamExt};
use log::{debug, info};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let _ = env_logger::try_init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    println!("Listening on: {}", addr);

    let (tx, rx) = unbounded();
    tokio::spawn(async move {
        while let Ok(i) = rx.recv_async().await {
            let mut batch = vec![i];
            let len = rx.len();
            for _l in 0..len {
                if let Ok(i) = rx.recv() {
                    batch.push(i);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            info!("batch received: {:?}", batch.len());
        }
    });

    loop {
        let tx_c = tx.clone();
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(async move {
                    let res = accept_connection(stream, tx_c).await;
                    if let Err(err) = res {
                        debug!("WebSocket error: {}", err);
                    }
                });
            }
            Err(err) => {
                debug!("Tcp error: {}", err);
                break;
            }
        }
    }
    Ok(())
}

async fn accept_connection(
    stream: TcpStream,
    tx: Sender<String>,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let addr = stream.peer_addr()?;
    debug!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream).await?;

    debug!("WebSocket connect: {}", addr);

    let (mut write, mut read) = ws_stream.split();
    // Wait message

    loop {
        let msg = read.next().await;
        match msg {
            Some(msg) => {
                let msg = msg?;
                if msg.is_text() {
                    let msg = msg.to_string();
                    let _ = tx.send(msg.clone());
                    if msg.contains("EVENT") {
                        write.send(Message::Text(r#"["OK", "b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30", true, ""]"#.to_string())).await?;
                    } else if msg.contains("REQ") {
                        write
                            .send(Message::Text(r#"["EOSE", "sub"]"#.to_string()))
                            .await?;
                    }
                } else if msg.is_close() {
                    debug!("WebSocket closed: {}", addr);
                    break;
                }
            }
            None => break,
        }
    }
    Ok(())
}
