use futures_util::{SinkExt, StreamExt, TryStreamExt};
use std::net::SocketAddr;
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

/// Parse interface string
pub fn parse_interface(s: &str) -> Result<SocketAddr, String> {
    Ok(format!("{}:0", s).parse().map_err(|_| "error format")?)
}

pub fn parse_wsaddr(url: &Url) -> std::io::Result<SocketAddr> {
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
