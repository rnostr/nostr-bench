use actix::prelude::*;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use log::{debug, error};
use std::{env, net::SocketAddr};

/// websocket connection is long running connection, it easier
/// to handle with an actor

pub struct MyWebSocket {
    addr: SocketAddr,
}

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, _ctx: &mut Self::Context) {
        debug!("start {}", self.addr);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("stop {}", self.addr);
    }
}

/// Handler for `ws::Message`
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        debug!("WEBSOCKET MESSAGE: {msg:?}");
        let msg = match msg {
            Err(err) => {
                error!("Receive error: {:?}", err);
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };
        match msg {
            ws::Message::Text(text) => {
                if text.contains("EVENT") {
                    ctx.text(r#"["OK", "b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30", true, ""]"#);
                } else if text.contains("REQ") {
                    ctx.text("[\"EVENT\",\"sub\",{\"content\":\"This is a message from nostr-bench client\",\"created_at\":1679398712,\"id\":\"7c3d4ede274a5ee7b4902ca0b1b0c66455668a726c3f13d0e4df98001d265ab2\",\"kind\":1,\"pubkey\":\"9995b312995668064f9418db06a99758e80d9b35e7a4e76cba899e5f7abc3614\",\"sig\":\"4de6d7457f6194122949f87fce5acaab37cef9867aca59980399dd0ab055a54f8cf22f277f3a91a6f5546c88e42c5abb87fac69839c64119b99cee679190a105\",\"tags\":[[\"p\",\"9995b312995668064f9418db06a99758e80d9b35e7a4e76cba899e5f7abc3614\"],[\"e\",\"378f145897eea948952674269945e88612420db35791784abf0616b4fed56ef7\"],[\"t\",\"nostr-bench-\"],[\"t\",\"nostr-bench-515\"]]}]");
                    ctx.text(r#"["EOSE", "sub"]"#);
                }
            }
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

/// WebSocket handshake and start `MyWebSocket` actor.
async fn echo_ws(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(
        MyWebSocket {
            addr: req.peer_addr().unwrap(),
        },
        &req,
        stream,
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = env_logger::try_init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    println!("Listening on: {}", addr);

    HttpServer::new(|| {
        App::new()
            // websocket route
            .service(web::resource("/").route(web::get().to(echo_ws)))
        // enable logger
        // .wrap(middleware::Logger::default())
    })
    // .workers(2)
    .bind(addr)?
    .run()
    .await
}
