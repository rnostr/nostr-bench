//! A simple async server.
//!
//! You can test this out by running:
//!
//!     cargo run --example server 127.0.0.1:12345
//!

use std::env;

use futures_util::{future, StreamExt, TryStreamExt};
use log::{error, info};
use tokio::net::{TcpListener, TcpStream};

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

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(async {
                    let res = accept_connection(stream).await;
                    if let Err(err) = res {
                        error!("WebSocket error: {}", err);
                    }
                });
            }
            Err(err) => {
                error!("Tcp error: {}", err);
                break;
            }
        }
    }
    Ok(())
}

async fn accept_connection(stream: TcpStream) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let addr = stream.peer_addr()?;
    info!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream).await?;

    info!("WebSocket connect: {}", addr);

    let (_write, read) = ws_stream.split();
    // Wait message
    let res = read.try_for_each(|_| future::ready(Ok(()))).await;
    match res {
        Ok(_) => {
            info!("WebSocket closed: {}", addr);
        }
        Err(err) => {
            info!("WebSocket error: {} {}", addr, err);
        }
    }
    Ok(())
}
