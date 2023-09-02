use specs::{Component, HashMapStorage};
use uuid::Uuid;


#[derive(Component, Default)]
#[storage(HashMapStorage)]
pub struct NetworkReplicated {
    pub net_id: Uuid
}
