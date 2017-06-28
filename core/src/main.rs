extern crate websocket;
extern crate futures;
extern crate futures_cpupool;
extern crate tokio_core;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;


use websocket::message::OwnedMessage;
use websocket::server::InvalidConnection;
use websocket::async::Server;

use tokio_core::reactor::{Handle, Core};

use futures::{Future, Sink, Stream};
use futures::future::{self, Loop};
use futures::sync::mpsc;
use futures_cpupool::CpuPool;

use std::sync::{RwLock, Arc};
use std::thread;
use std::rc::Rc;
use std::fmt::Debug;
use std::time::Duration;
use std::ops::Deref;



fn main() {
    let mut core = Core::new().expect("Failed to create Tokio event loop");
    let handle = core.handle();
    let remote = core.remote();
    let server = Server::bind("localhost:8081", &handle).expect("Failed to create server");
    let pool = Rc::new(CpuPool::new_num_cpus());
    let connections = Arc::new(RwLock::new(Vec::new()));
    let state = Arc::new(RwLock::new(State::new()));
    let (read_channel_out, read_channel_in) = mpsc::unbounded();
    let connections_inner = connections.clone();
    let connection_handler = server.incoming()
        // we don't wanna save the stream if it drops
        .map_err(|InvalidConnection { error, .. }| error)
        .for_each(move |(upgrade, addr)| {
            let connections_inner = connections_inner.clone();
            println!("Got a connection from: {}", addr);
            let channel = read_channel_out.clone();
            let handle_inner = handle.clone();
            let f = upgrade
                .use_protocol("rust-websocket")
                .accept()
                .and_then(move |(framed, _)| {
                    let (sink, stream) = framed.split();
                    let f = channel.send(stream);
                    spawn_future(f, "Senk sink to connection pool", &handle_inner);
                    connections_inner.write().unwrap().push(Arc::new(RwLock::new(sink)));
                    Ok(())
                });
            spawn_future(f, "Handle new connection", &handle);
            Ok(())
        })
        .map_err(|_| ());


    let state_read = state.clone();
    let remote_write = remote.clone();
    let read_handler = pool.spawn_fn(|| {
        read_channel_in.for_each(move |stream| {
            let state_read = state_read.clone();
            remote_write.spawn(|_| {
                stream.for_each(move |msg| {
                                    handle_incoming(&mut state_read.write().unwrap(), &msg);
                                    Ok(())
                                })
                    .map_err(|_| ())
            });
            Ok(())
        })
    });

    let (write_channel_out, write_channel_in) = futures::sync::mpsc::unbounded();

    type MessageCodec = websocket::async::MessageCodec<OwnedMessage>;
    type Framed = websocket::client::async::Framed<tokio_core::net::TcpStream, MessageCodec>;
    type SplitSink = futures::stream::SplitSink<Framed>;
    let write_handler = pool.spawn_fn(move || {
        write_channel_in.for_each(move |(sink, state): (Arc<RwLock<SplitSink>>,
                                                        Arc<RwLock<State>>)| {
                let msg = serde_json::to_string(state.read().unwrap().deref()).unwrap();
                println!("Sending message: {}", msg);
                sink.write()
                    .unwrap()
                    .start_send(OwnedMessage::Text(msg))
                    .unwrap();
                sink.write()
                    .unwrap()
                    .poll_complete()
                    .unwrap();
                Ok(())
            })
            .map_err(|_| ())
    });

    let game_loop = pool.spawn_fn(move || {
        future::loop_fn(write_channel_out, move |write_channel_out| {
            thread::sleep(Duration::from_millis(100));
            let connections = connections.clone();
            let state = state.clone();
            let write_channel_out_inner = write_channel_out.clone();
            remote.spawn(move |handle| {
                for conn in connections.write().unwrap().iter() {
                    let f = write_channel_out_inner.clone().send((conn.clone(), state.clone()));
                    spawn_future(f, "Send message to client", handle);
                }
                Ok(())
            });

            // insert your terminating condition here
            match 1 {
                1 => Ok(Loop::Continue(write_channel_out)),
                2 => Ok(Loop::Break(())),
                _ => Err(()),
            }
        })
    });

    let handlers =
        game_loop.select2(connection_handler.select2(read_handler.select(write_handler)));
    core.run(handlers).map_err(|_| println!("err")).unwrap();
}

fn spawn_future<F, I, E>(f: F, desc: &'static str, handle: &Handle)
    where F: Future<Item = I, Error = E> + 'static,
          E: Debug
{
    handle.spawn(f.map_err(move |e| println!("Error in {}: '{:?}'", desc, e))
                     .map(move |_| println!("{}: Finished.", desc)));
}


fn handle_incoming(_: &mut State, msg: &OwnedMessage) {
    if let OwnedMessage::Text(ref txt) = *msg {
        println!("Received message: {}", txt);
        //state.msg = txt.clone();
    }
}


#[derive(Serialize, Deserialize)]
struct State {
    positions: Vec<Pos>,
}

impl State {
    fn new() -> Self {
        Self { positions: Vec::new() }
    }
}


#[derive(Serialize, Deserialize)]
struct Pos {
    x: i32,
    y: i32,
}

/*
impl Pos {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn add_x(&mut self, x: i32) -> &mut Self {
        self.x += x;
        self
    }


    fn add_y(&mut self, y: i32) -> &mut Self {
        self.y += y;
        self
    }

    fn add(&mut self, x: i32, y: i32) -> &mut Self {
        self.x += x;
        self.y += y;
        self
    }
}
*/
