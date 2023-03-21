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
                        write.send(Message::Text("[\"EVENT\",\"sub\",{\"content\":\"This is a message from nostr-bench client\",\"created_at\":1679398712,\"id\":\"7c3d4ede274a5ee7b4902ca0b1b0c66455668a726c3f13d0e4df98001d265ab2\",\"kind\":1,\"pubkey\":\"9995b312995668064f9418db06a99758e80d9b35e7a4e76cba899e5f7abc3614\",\"sig\":\"4de6d7457f6194122949f87fce5acaab37cef9867aca59980399dd0ab055a54f8cf22f277f3a91a6f5546c88e42c5abb87fac69839c64119b99cee679190a105\",\"tags\":[[\"p\",\"9995b312995668064f9418db06a99758e80d9b35e7a4e76cba899e5f7abc3614\"],[\"e\",\"378f145897eea948952674269945e88612420db35791784abf0616b4fed56ef7\"],[\"t\",\"nostr-bench-\"],[\"t\",\"nostr-bench-515\"]]}]".to_owned())).await?;
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
