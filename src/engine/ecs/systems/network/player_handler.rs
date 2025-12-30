use std::collections::HashMap;

use log::{error, warn};
use nalgebra::{UnitQuaternion, UnitVector3};
use serde::{Deserialize, Serialize};
use specs::{Join, Read, ReadStorage, System, WorldExt as _, WriteStorage};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ecs::{
    components::{
        general::{Movement, PlayerInputFlags, Transform},
        network::NetworkReplicated,
    },
    resources::network::{
        MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut, NetworkProtocol,
    },
};

#[derive(Serialize, Deserialize)]
struct PlayerInputData {
    pub entity_id: Uuid,
    pub rotation: UnitQuaternion<f32>,
    pub input: PlayerInputFlags,
}

/// Handler for network actions related to players
/// Spawns the player, handles player actions

pub struct PlayerHandler {
    receiver: broadcast::Receiver<NetworkPacketIn>,
}

impl Default for PlayerHandler {
    fn default() -> Self {
        PlayerHandler {
            receiver: broadcast::channel(1).1,
        }
    }
}

impl<'a> System<'a> for PlayerHandler {
    type SystemData = (
        ReadStorage<'a, NetworkReplicated>,
        WriteStorage<'a, Movement>,
        ReadStorage<'a, Transform>,
        Option<Read<'a, NetworkData>>,
    );

    fn run(
        &mut self,
        (network_replicated, mut movement, transform, network_data): Self::SystemData,
    ) {
        let net_data = match network_data {
            Some(v) => v,
            None => {
                warn!("No network data struct, cannot use networking.");
                return;
            }
        };

        // value is owner_id, input data
        let mut input_updates = HashMap::<Uuid, (Uuid, PlayerInputData)>::new();

        while !self.receiver.is_empty() {
            match self.receiver.try_recv() {
                Ok(data) => {
                    match data.message_type {
                        MessageType::PlayerInput => {
                            if net_data.is_server {
                                match rmp_serde::from_slice::<PlayerInputData>(&data.data) {
                                    Ok(t) => {
                                        input_updates
                                            .insert(t.entity_id, (data.client.client_id, t));
                                    }
                                    Err(e) => error!("Could not parse Transform: {:?}", e),
                                }
                            }
                        }
                        _ => {} // dont care
                    }
                }
                Err(e) => {
                    error!("Error receiving in GenericHandler: {:?}", e);
                }
            }
        }

        for (net_rep, m, t) in (&network_replicated, &mut movement, &transform).join() {
            if net_rep.net_id.is_nil() {
                error!("Tried to update a network replicated entity with respect to movement, which did not have a valid net_id. Ignoring");
                continue;
            }

            if net_data.is_server {
                if let Some(input) = input_updates.get(&net_rep.net_id) {
                    if net_rep.owner_id != input.0 {
                        error!(
                            "Client {:?} tried to control entity {:?} not owned by them",
                            input.0, net_rep.net_id
                        );
                        continue;
                    }

                    let i = &input.1;

                    m.req_rotation = Some(i.rotation);
                    m.req_movement = Some(i.input);
                }
            } else {
                if net_data
                    .player_self
                    .is_some_and(|x| x.client_id == net_rep.owner_id)
                {
                    match rmp_serde::to_vec(&PlayerInputData {
                        entity_id: net_rep.net_id,
                        rotation: m.req_rotation.unwrap_or(t.rot),
                        input: m.req_movement.unwrap_or_default(),
                    }) {
                        Ok(v) => {
                            if let Err(e) = net_data.sender.try_send(NetworkPacketOut {
                                net_id: net_rep.net_id,
                                message_type: MessageType::PlayerInput,
                                protocol: NetworkProtocol::UDP,
                                data: v,
                            }) {
                                error!("Could not send to tokio PlayerInput: {:?}", e);
                            }
                        }
                        Err(e) => {
                            error!("Could not convert PlayerInputData to vec: {:?}", e);
                        }
                    }
                }
            }
        }
    }

    fn setup(&mut self, world: &mut specs::World) {
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
