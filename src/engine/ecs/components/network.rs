use specs::{Component, HashMapStorage};
use uuid::Uuid;

#[derive(Component, Default)]
#[storage(HashMapStorage)]
pub struct NetworkReplicated {
    // id of the object
    pub net_id: Uuid,
    // id of the player/server who owns this object
    pub owner_id: Uuid,
    // type of the entity, freeform string
    // used to identify on the game side which
    // entity to spawn
    pub entity_type: String,
}
