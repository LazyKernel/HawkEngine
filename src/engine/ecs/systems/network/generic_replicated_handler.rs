use std::collections::HashMap;

use log::{error, warn};
use serde::{Deserialize, Serialize};
use specs::{
    shred::DynamicSystemData, Join, Read, ReadStorage, System, WorldExt as _, WriteStorage,
};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ecs::{
    components::{general::Transform, network::NetworkReplicated},
    resources::network::{
        MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut, NetworkProtocol,
    },
};

#[derive(Serialize, Deserialize)]
struct TransformMessage {
    pub component_id: Uuid,
    pub transform: Transform,
}

#[derive(Serialize, Deserialize)]
struct NewReplicatedMessage {
    pub object_type: String,
    pub init_transform: Transform,
}

/// Handler for generic replicated components
/// Responsible for converting Transform updates to network messages
///

pub struct GenericHandler {
    receiver: broadcast::Receiver<NetworkPacketIn>,
}

impl Default for GenericHandler {
    fn default() -> Self {
        GenericHandler {
            receiver: broadcast::channel(1).1,
        }
    }
}

impl<'a> System<'a> for GenericHandler {
    type SystemData = (
        ReadStorage<'a, NetworkReplicated>,
        WriteStorage<'a, Transform>,
        Option<Read<'a, NetworkData>>,
    );

    fn run(&mut self, (network_replicated, mut transform, network_data): Self::SystemData) {
        let net_data = match network_data {
            Some(v) => v,
            None => {
                warn!("No network data struct, cannot use networking.");
                return;
            }
        };

        let mut transform_updates = HashMap::<Uuid, Transform>::new();

        while !self.receiver.is_empty() {
            match self.receiver.try_recv() {
                Ok(data) => {
                    // NOTE: all data we would receive here are for clients only
                    // server should never trust the clients with pure transform
                    // or new object creation messages
                    if net_data.is_server {
                        continue;
                    }

                    match data.message_type {
                        MessageType::ComponentTransform => {
                            match rmp_serde::from_slice::<TransformMessage>(&data.data) {
                                Ok(t) => {
                                    transform_updates.insert(t.component_id, t.transform);
                                }
                                Err(e) => error!("Could not parse Transform: {:?}", e),
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

        for (net_rep, t) in (&network_replicated, &mut transform).join() {
            if net_rep.net_id.is_nil() {
                error!("Tried to update a network replicated entity with respect to transform, which did not have a valid net_id. Ignoring");
                continue;
            }

            if net_data.is_server {
                match rmp_serde::to_vec(&TransformMessage {
                    component_id: net_rep.net_id,
                    transform: *t,
                }) {
                    Ok(v) => {
                        let message = NetworkPacketOut {
                            net_id: net_rep.net_id,
                            message_type: MessageType::ComponentTransform,
                            data: v,
                            protocol: NetworkProtocol::UDP,
                            ..Default::default()
                        };

                        if let Err(e) = net_data.sender.try_send(message) {
                            error!("Failed sending from GenericHandler to tokio: {:?}", e)
                        }
                    }
                    Err(e) => error!("Could not serialize transform: {e}"),
                };
            } else {
                if let Some(trans) = transform_updates.get(&net_rep.net_id) {
                    *t = *trans;
                }
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
