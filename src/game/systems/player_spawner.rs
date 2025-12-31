use std::time::Instant;

use engine::ecs::resources::network::MessageType;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use specs::{shred::DynamicSystemData, System, WorldExt, Write};
use tokio::sync::broadcast;
use uuid::Uuid;

use engine::{
    ecs::resources::network::{
        MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut, NetworkProtocol, Player,
    },
    ecs::systems::network::connection_handler::NewClientData,
};

// Spawns a new player on new client join

pub struct PlayerSpawner {
    receiver: broadcast::Receiver<NetworkPacketIn>,
}

impl Default for PlayerSpawner {
    fn default() -> Self {
        PlayerSpawner {
            receiver: broadcast::channel(1).1,
        }
    }
}

impl<'a> System<'a> for PlayerSpawner {
    type SystemData = (Option<Write<'a, NetworkData>>,);

    fn run(&mut self, (network_data,): Self::SystemData) {
        let net_data = match network_data {
            Some(v) => v,
            None => {
                warn!("No network data struct, cannot use networking.");
                return;
            }
        };

        // handle incoming packets
        while !self.receiver.is_empty() {
            match self.receiver.try_recv() {
                Ok(v) => match v.message_type {
                    MessageType::NewClient => {
                        match rmp_serde::from_slice::<NewClientData>(&v.data) {
                            Ok(data) => {
                                if data.uuid != net_data.player_self.unwrap_or_default().client_id {
                                    // TODO: create player
                                } else {
                                    // TODO: this is our player, add network replicated component
                                    // to it
                                }
                            }
                            Err(e) => {
                                error!("Could not parse NewClientData in PlayerSpawner: {:?}", e)
                            }
                        }
                    }
                    _ => {} // we dont care
                },
                Err(e) => error!("Failed receiving net data in ConnectionHandler: {:?}", e),
            }
        }
    }

    fn setup(&mut self, world: &mut specs::World) {
        <Self::SystemData as DynamicSystemData>::setup(&self.accessor(), world);
        let net_data = world.read_resource::<NetworkData>();
        self.receiver = net_data.in_packets_sender.subscribe();
    }

    fn dispose(self, world: &mut specs::World)
    where
        Self: Sized,
    {
        drop(self.receiver);
    }
}
