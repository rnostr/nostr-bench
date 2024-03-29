use nostr::prelude::{
    rand,
    rand::distributions::{Alphanumeric, DistString},
    rand::Rng,
};
use std::net::SocketAddr;
use url::Url;

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

/// Generate random hashtag between nostr-bench-0 to nostr-bench-1000
pub fn gen_hashtag() -> String {
    let mut prefix = "nostr-bench-".to_owned();
    let mut rng = rand::thread_rng();
    prefix.push_str(&rng.gen_range(0..1000).to_string());
    prefix
}

/// Generate request
pub fn gen_req(id: Option<String>, tag: Option<String>, limit: usize) -> String {
    let id = id.unwrap_or("sub".to_owned());
    let tag = tag.unwrap_or_else(|| gen_hashtag());
    format!(
        "[\"REQ\", \"{}\", {{\"#t\": [\"{}\"], \"limit\": {}}}]",
        id, tag, limit
    )
}

/// Generate close message
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

pub fn gen_string(size: usize) -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), size)
}

#[cfg(test)]
mod tests {
    use crate::util::gen_string;

    use super::{gen_close, gen_req};
    #[test]
    fn generate() {
        assert_eq!(
            gen_req(Some("id".to_owned()), Some("tag".to_owned()), 1),
            r###"["REQ", "id", {"#t": ["tag"], "limit": 1}]"###
        );
        assert_eq!(gen_close(Some("id".to_owned())), r#"["CLOSE", "id"]"#);
        assert_eq!(gen_string(10).len(), 10);
    }
}
