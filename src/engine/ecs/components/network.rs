use specs::{Component, HashMapStorage};
use uuid::Uuid;

#[derive(Component, Default)]
#[storage(HashMapStorage)]
pub struct NetworkReplicated {
    // id of the object
    pub net_id: Uuid,
    // id of the player/server who owns this object
    pub owner_id: Uuid
}
