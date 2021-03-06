extern crate shootr;

extern crate specs;
extern crate chrono;
extern crate serde_json;
extern crate websocket_server;
extern crate dotenv;

use specs::{DispatcherBuilder, World, Entity};
use chrono::prelude::*;
use websocket_server::{start as start_server, EventHandler, SendChannel, Message};
use dotenv::dotenv;

use shootr::util::{read_env_var, elapsed_ms, SeqIdGen};
use shootr::model::comp::{ToSpawn, ToDespawn, Player, Actor, ActorKind};
use shootr::model::network::ClientMsg;
use shootr::model::game::Id;
use shootr::system::*;
use shootr::bootstrap;
use shootr::collision::World as CollisionWorld;

use std::sync::{Arc, RwLock};
use std::thread::sleep;
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

fn main() {
    dotenv().ok();
    let port = read_env_var("CORE_PORT").parse::<u32>().expect(
        "Specified port is not a valid number",
    );
    start_server::<Handler>("localhost", port);
}

struct Handler {
    id_entity: RwLock<HashMap<Id, Entity>>,
    to_spawn: RwLock<HashMap<Id, SendChannel>>,
    to_despawn: RwLock<HashSet<Id>>,
    inputs: Arc<RwLock<HashMap<Id, Vec<ClientMsg>>>>,
}

impl Handler {
    fn prepare_world(&self, world: &mut World) {
        bootstrap::prepare_world(world);
        world.add_resource(self.inputs.clone());
        world.add_resource(RwLock::new(CollisionWorld::<Id>::new(1000, 1000)));

        // Create ball
        let id = Id::new_v4();
        let entity = world
            .create_entity()
            .with(ToSpawn {})
            .with(Actor {
                id,
                kind: ActorKind::Ball,
            })
            .build();
        self.id_entity.write().unwrap().insert(id, entity);
    }


    fn handle_msg(&self, id: Id, msg: &str) {
        if let Ok(key_state) = serde_json::from_str::<ClientMsg>(msg) {
            let mut inputs = self.inputs.write().unwrap();
            let has_already_inputs = inputs.get(&id).is_some();
            if has_already_inputs {
                inputs.get_mut(&id).unwrap().push(key_state);
            } else {
                inputs.insert(id, vec![key_state]);
            }
        } else {
            println!("Client {}: Sent invalid message: {}", id, msg);
        }
    }

    fn register_connections(&self, world: &mut World) {
        let mut id_entity = self.id_entity.write().unwrap();
        let mut to_spawn = self.to_spawn.write().unwrap();
        for (id, send_channel) in to_spawn.drain() {
            let entity = world
                .create_entity()
                .with(ToSpawn {})
                .with(Player::new(send_channel))
                .with(Actor {
                    id,
                    kind: ActorKind::Player,
                })
                .build();
            id_entity.insert(id, entity);
        }

        let mut to_despawn = self.to_despawn.write().unwrap();
        for id in to_despawn.drain() {
            if let Some(entity) = id_entity.remove(&id) {
                world.write::<ToDespawn>().insert(entity, ToDespawn {});
            }
        }
    }
}

impl EventHandler for Handler {
    type Id = Id;

    fn new() -> Self {
        Handler {
            id_entity: RwLock::new(HashMap::new()),
            to_spawn: RwLock::new(HashMap::new()),
            to_despawn: RwLock::new(HashSet::new()),
            inputs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    fn main_loop(&self) {
        let mut world = World::new();
        self.prepare_world(&mut world);

        let mut updater = DispatcherBuilder::new()
            .add(InputHandler, "input_handler", &[])
            .add(Spawn, "spawn", &["input_handler"])
            .add(Physics, "physics", &["spawn"])
            .add(Bounce, "bounce", &["physics"])
            .build();
        // Initial update
        updater.dispatch(&mut world.res);

        let mut sender = DispatcherBuilder::new()
            .add(Sending, "sending", &[])
            .add(Despawn, "despawn", &["sending"])
            .build();

        let mut lag: u64 = 0;
        let mut previous = Utc::now();
        let updates_per_sec = read_env_var("CORE_UPDATES_PER_SEC").parse::<u64>().expect(
            "Failed to parse environmental variable as integer",
        );
        let ms_per_update = 1000 / updates_per_sec;
        let mut curr_tick_generator = SeqIdGen::default();
        loop {
            let current = Utc::now();
            let elapsed = elapsed_ms(previous, current).expect("Time went backwards");
            previous = current;
            lag += elapsed;
            world.add_resource(curr_tick_generator.gen());

            self.register_connections(&mut world);
            while lag >= ms_per_update {
                updater.dispatch(&mut world.res);
                world.maintain();
                lag -= ms_per_update;
            }
            sender.dispatch(&mut world.res);

            sleep(Duration::from_millis(ms_per_update - lag));
        }
    }

    fn on_message(&self, id: Self::Id, msg: Message) {
        if let Message::Text(ref txt) = msg {
            self.handle_msg(id, txt);
        };
    }
    fn on_connect(&self, _: SocketAddr, send_channel: SendChannel) -> Option<Self::Id> {
        let id = Id::new_v4();
        self.to_spawn.write().unwrap().insert(id, send_channel);
        println!("Client {}: Connected", id);
        Some(id)
    }
    fn on_disconnect(&self, id: Self::Id) {
        println!("Client {}: Disconnected", id);
        self.to_despawn.write().unwrap().insert(id);
    }
}
