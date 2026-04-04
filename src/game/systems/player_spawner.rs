use std::time::Instant;

use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use specs::{shred::DynamicSystemData, ReadStorage, System, WorldExt, Write};
use tokio::sync::broadcast;
use uuid::Uuid;

use engine::ecs::{
    components::network::NetworkReplicated,
    resources::{
        network::{
            MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut, NetworkProtocol,
            NewReplicatedData, Player,
        },
        ActiveCamera,
    },
    systems::network::connection_handler::NewClientData,
};

use crate::create_player;

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
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, ActiveCamera>,
        Option<Write<'a, NetworkData>>,
    );

    fn run(&mut self, (entities, activecam, network_data): Self::SystemData) {
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
                                if net_data.is_server {
                                    let id = create_player(world, physics_data, renderer, false);
                                    entities.get(id).insert(NetworkReplicated {
                                        owner_id: data.uuid,
                                        net_id: Uuid::new_v4(),
                                        entity_type: "Player".into(),
                                    });
                                }
                            }
                            Err(e) => {
                                error!("Could not parse NewClientData in PlayerSpawner: {:?}", e)
                            }
                        }
                    }
                    MessageType::NewReplicated => {
                        match rmp_serde::from_slice::<NewReplicatedData>(&v.data) {
                            Ok(data) => {
                                if net_data.is_server {
                                    // server has already created any replicated entities
                                    continue;
                                }

                                match data.entity_type.as_str() {
                                    "Player" => {
                                        if data.owner_id
                                            == net_data.player_self.unwrap_or_default().client_id
                                        {
                                            for (e, c) in (&entities, &activecam).join() {
                                                e.insert(NetworkReplicated {
                                                    owner_id: data.owner_id,
                                                    net_id: data.entity_id,
                                                    entity_type: data.entity_type,
                                                });
                                            }
                                        } else {
                                            create_player(world, physics_data, renderer, false);
                                        }
                                    }
                                    _ => {} // ignore others
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Could not parse NewReplicatedData in PlayerSpawner: {:?}",
                                    e
                                )
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
