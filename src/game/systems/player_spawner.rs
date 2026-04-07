use std::time::Instant;

use log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use specs::{
    shred::DynamicSystemData, Builder, Entities, Join as _, LazyUpdate, Read, ReadStorage, System,
    WorldExt, Write, WriteStorage,
};
use tokio::sync::broadcast;
use uuid::Uuid;

use engine::ecs::{
    components::{general::LocalPlayer, network::NetworkReplicated},
    resources::{
        network::{
            MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut, NetworkProtocol,
            NewReplicatedData, Player,
        },
        physics::PhysicsData,
        ActiveCamera, RenderData,
    },
    systems::network::connection_handler::NewClientData,
};

use crate::{create_player, create_player_components, TempRenderInputChoice};

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
        WriteStorage<'a, NetworkReplicated>,
        ReadStorage<'a, LocalPlayer>,
        Option<Write<'a, NetworkData>>,
        Option<Write<'a, PhysicsData>>,
        Option<Read<'a, RenderData>>,
        Read<'a, LazyUpdate>,
    );

    fn run(
        &mut self,
        (
            entities,
            mut netreplicated,
            localplayer,
            network_data,
            physics_data,
            render_data,
            lazy,
        ): Self::SystemData,
    ) {
        let net_data = match network_data {
            Some(v) => v,
            None => {
                warn!("No network data struct, cannot use networking.");
                return;
            }
        };

        let mut phys_data = match physics_data {
            Some(v) => v,
            None => {
                warn!("No physics data struct, cannot create players in player spawner");
                return;
            }
        };

        let rend_data = match render_data {
            Some(v) => v,
            None => {
                warn!("No render data struct, cannot create players in player spawner");
                return;
            }
        };

        // handle incoming packets
        while !self.receiver.is_empty() {
            match self.receiver.try_recv() {
                Ok(v) => match v.message_type {
                    MessageType::InitGameStateRequest => {
                        if net_data.is_server {
                            info!("Creating new player on server");
                            let new_entity =
                                lazy.create_entity(&entities).with(NetworkReplicated {
                                    owner_id: v.client.client_id,
                                    net_id: Uuid::new_v4(),
                                    entity_type: "Player".into(),
                                });
                            let _ = create_player(
                                new_entity,
                                &mut phys_data,
                                TempRenderInputChoice::RENDERDATA(&rend_data),
                            );
                        }
                    }
                    MessageType::NewReplicated => {
                        match rmp_serde::from_slice::<NewReplicatedData>(&v.data) {
                            Ok(data) => {
                                if net_data.is_server {
                                    // server has already created any replicated entities
                                    continue;
                                }

                                trace!("New replicated message on client");

                                match data.entity_type.as_str() {
                                    "Player" => {
                                        if data.owner_id
                                            == net_data.player_self.unwrap_or_default().client_id
                                        {
                                            for (e, _) in (&*entities, &localplayer).join() {
                                                if let Err(err) = netreplicated.insert(
                                                    e,
                                                    NetworkReplicated {
                                                        owner_id: data.owner_id,
                                                        net_id: data.entity_id,
                                                        entity_type: data.entity_type.clone(),
                                                    },
                                                ) {
                                                    error!("Could not insert NetworkReplicated to own player: {:?}", err);
                                                }
                                            }
                                        } else {
                                            let new_entity = lazy.create_entity(&entities).with(
                                                NetworkReplicated {
                                                    owner_id: data.owner_id,
                                                    net_id: data.entity_id,
                                                    entity_type: data.entity_type,
                                                },
                                            );
                                            let _ = create_player(
                                                new_entity,
                                                &mut phys_data,
                                                TempRenderInputChoice::RENDERDATA(&rend_data),
                                            );
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
