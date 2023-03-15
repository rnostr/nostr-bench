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
