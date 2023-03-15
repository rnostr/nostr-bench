use futures_util::Future;
use nostr::prelude::{rand, rand::Rng};
use std::net::SocketAddr;
use tokio::{
    net::{TcpSocket, TcpStream},
    time,
    time::Duration,
};
use tokio_tungstenite::{client_async, tungstenite::Error as WsError, WebSocketStream};
use url::Url;

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

/// Timeout error
pub async fn timeout<T: Future<Output = Result<(), Error>>>(
    timeout: u64,
    stay: T,
) -> Result<(), Error> {
    let result = if timeout == 0 {
        Ok(stay.await)
    } else {
        time::timeout(Duration::from_secs(timeout), stay)
            .await
            .map_err(|_| Error::AliveTimeout)
    };
    result?.map_err(|_| Error::Lost)?;
    Ok(())
}

/// Generate random hashtag between nostr-bench-0 to nostr-bench-1000
pub fn gen_hashtag() -> String {
    let mut prefix = "nostr-bench-".to_owned();
    let mut rng = rand::thread_rng();
    prefix.push_str(&rng.gen_range(0..1000).to_string());
    prefix
}

/// Generate request
pub fn gen_req(id: Option<String>, tag: Option<String>) -> String {
    let id = id.unwrap_or("sub".to_owned());
    let tag = tag.unwrap_or_else(|| gen_hashtag());
    format!(
        "[\"REQ\", \"{}\", {{\"#t\": [\"{}\"], \"limit\": 1}}]",
        id, tag
    )
}

pub fn gen_close(id: Option<String>) -> String {
    let id = id.unwrap_or("sub".to_owned());
    format!("[\"CLOSE\", \"{}\"]", id)
}

/// Generate random note with different key
pub fn gen_note_event<T: Into<String>>(content: T) -> String {
    let key = nostr::Keys::generate();
    let tags = vec![
        nostr::Tag::PubKey(key.public_key(), None),
        nostr::Tag::Event(
            nostr::EventId::from_hex(
                "378f145897eea948952674269945e88612420db35791784abf0616b4fed56ef7",
            )
            .unwrap(),
            None,
            None,
        ),
        nostr::Tag::Hashtag("nostr-bench-".to_owned()),
        nostr::Tag::Hashtag(gen_hashtag()),
    ];
    let builder = nostr::EventBuilder::new_text_note(content, &tags);
    let event = builder.to_event(&key).unwrap();
    nostr::ClientMessage::new_event(event).as_json()
}

#[cfg(test)]
mod tests {
    use super::{gen_close, gen_req};
    #[test]
    fn generate() {
        assert_eq!(
            gen_req(Some("id".to_owned()), Some("tag".to_owned())),
            r###"["REQ", "id", {"#t": ["tag"], "limit": 1}]"###
        );
        assert_eq!(gen_close(Some("id".to_owned())), r#"["CLOSE", "id"]"#);
    }
}
