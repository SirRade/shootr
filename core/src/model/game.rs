use engine::Id;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use super::client::Key;

#[derive(Debug, Clone, Serialize)]
pub struct KeyState {
    pub pressed: bool,
    pub fired: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bounds<T> {
    pub max: T,
    pub min: T,
}

#[derive(Debug, Clone, Serialize)]
pub enum Spawnable {
    Player(Id)
}

pub type PlayerInputMap = Arc<RwLock<HashMap<Id, RwLock<PlayerInput>>>>;
#[derive(Debug, Clone, Serialize)]
pub struct PlayerInput {
    pub key_states: HashMap<Key, KeyState>,
}