extern crate websocket;
extern crate futures;
extern crate tokio_core;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::fmt::Debug;

use websocket::message::OwnedMessage;
use websocket::server::InvalidConnection;
use websocket::client::async::Framed;
use websocket::async::{Server, MessageCodec};
use websocket::WebSocketError;

use tokio_core::net::TcpStream;
use tokio_core::reactor::{Handle, Core};

use futures::{Future, Sink, Stream};
use futures::future::{self, Loop};

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let server = Server::bind("localhost:8081", &handle).unwrap();

    let f = server.incoming()
        // we don't wanna save the stream if it drops
        .map_err(|InvalidConnection { error, .. }| error)
        .for_each(|(upgrade, addr)| {
            println!("Got a connection from: {}", addr);
            if !upgrade.protocols().iter().any(|s| s == "rust-websocket") {
                spawn_future(upgrade.reject(), "Upgrade Rejection", &handle);
                return Ok(());
            }

            let client = upgrade
                .use_protocol("rust-websocket")
                .accept();
            let f = client
                .and_then(|(framed, _)| {
                    let (sink, stream) = framed.split();
                   
                    let input = stream
                            .for_each(|msg|{
                                println!("0");
                                let mut state = State::new();
                                handle_incoming(&mut state, &msg);
                                Ok(())
                            })
                            .map_err(|e| WebSocketError::NoDataAvailable);

                    let output = sink
                            .send({
                                println!("1");
                                OwnedMessage::Text("hi!".to_owned())
                            })
                            .map_err(|e| WebSocketError::NoDataAvailable);
                    input.join(output)
                });
            spawn_future(f, "Client Status", &handle);
            Ok(())
        });


    core.run(f).unwrap();
}

fn spawn_future<F, I, E>(f: F, desc: &'static str, handle: &Handle)
    where F: Future<Item = I, Error = E> + 'static,
          E: Debug
{
    handle.spawn(f.map_err(move |e| println!("Error in {}: '{:?}'", desc, e))
                     .map(move |_| println!("{}: Finished.", desc)));
}


fn handle_incoming(state: &mut State, msg: &OwnedMessage) {
    println!("handle_incoming");
    match msg {
        &OwnedMessage::Text(ref txt) => {
            println!("Received message: {}", txt);
            state.msg = txt.clone();
        }
        _ => {}
    }
}

type FramedStream = Framed<TcpStream, MessageCodec<OwnedMessage>>;

fn send(state: &State, stream: FramedStream) -> futures::sink::Send<FramedStream> {
    let msg = serde_json::to_string(&state).unwrap();
    println!("Sending message: {}", msg);
    stream.send(OwnedMessage::Text(msg))
}


#[derive(Serialize)]
struct State {
    msg: String,
}

impl State {
    fn new() -> State {
        State { msg: "git gud".to_string() }
    }
}


