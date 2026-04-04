use std::collections::HashMap;

use log::{error, info, warn};
use specs::{shred::DynamicSystemData, Join, Read, ReadStorage, System, WorldExt, Write};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    ecs::{
        resources::network::{
            MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut, NetworkProtocol,
            NetworkTarget, NewReplicatedData, Player,
        },
        systems::network::connection_handler::NewClientData,
    },
    send_or_log_err,
};

use crate::ecs::components::network::NetworkReplicated;

pub struct NetReplicatedHandler {
    receiver: broadcast::Receiver<NetworkPacketIn>,
    netreplicateds: HashMap<Uuid, NetworkReplicated>,
}

impl Default for NetReplicatedHandler {
    fn default() -> Self {
        NetReplicatedHandler {
            receiver: broadcast::channel(1).1,
            netreplicateds: HashMap::new(),
        }
    }
}

impl<'a> System<'a> for NetReplicatedHandler {
    type SystemData = (
        ReadStorage<'a, NetworkReplicated>,
        Option<Write<'a, NetworkData>>,
    );

    fn run(&mut self, (net_replicateds, network_data): Self::SystemData) {
        let net_data = match network_data {
            Some(v) => v,
            None => {
                warn!("No network data struct, cannot use networking.");
                return;
            }
        };

        if !net_data.is_server {
            // we're not the server, we don't have to care
            return;
        }

        for nr in (&net_replicateds).join() {
            if !self.netreplicateds.contains_key(&nr.net_id) {
                // new net replicated, send over
                self.netreplicateds.insert(nr.net_id, nr.clone());
                send_or_log_err!(
                    net_data.sender,
                    &NewReplicatedData {
                        owner_id: nr.owner_id,
                        entity_id: nr.net_id,
                        entity_type: nr.entity_type.clone(),
                    },
                    NetworkTarget::Broadcast,
                    MessageType::NewReplicated,
                    NetworkProtocol::TCP
                );
            }
        }

        // handle incoming packets
        while !self.receiver.is_empty() {
            match self.receiver.try_recv() {
                Ok(v) => match v.message_type {
                    MessageType::NewClient => {
                        match rmp_serde::from_slice::<NewClientData>(&v.data) {
                            Ok(data) => {
                                for nr in self.netreplicateds.values() {
                                    send_or_log_err!(
                                        net_data.sender,
                                        &NewReplicatedData {
                                            owner_id: nr.owner_id,
                                            entity_id: nr.net_id,
                                            entity_type: nr.entity_type.clone()
                                        },
                                        NetworkTarget::Client(data.uuid),
                                        MessageType::NewReplicated,
                                        NetworkProtocol::TCP
                                    );
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Could not parse NewClientData in NetReplicatedHandler: {:?}",
                                    e
                                )
                            }
                        }
                    }
                    _ => {} // we dont care
                },
                Err(e) => error!("Failed receiving net data in NetReplicatedHandler: {:?}", e),
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
