use std::time::Instant;

use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use specs::{shred::DynamicSystemData, System, WorldExt, Write};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    ecs::resources::network::{
        MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut, NetworkProtocol, Player,
    },
    network::constants::KEEP_ALIVE_INTERVAL,
};

#[derive(Serialize, Deserialize)]
pub struct ConnectionAcceptData {
    pub uuid: Uuid,
}

#[derive(Serialize, Deserialize)]
pub struct NewClientData {
    pub name: String,
    pub uuid: Uuid,
}

// Starts and keeps the connection alive

pub struct ConnectionHandler {
    receiver: broadcast::Receiver<NetworkPacketIn>,
}

impl Default for ConnectionHandler {
    fn default() -> Self {
        ConnectionHandler {
            receiver: broadcast::channel(1).1,
        }
    }
}

impl<'a> System<'a> for ConnectionHandler {
    type SystemData = (Option<Write<'a, NetworkData>>,);

    fn run(&mut self, (network_data,): Self::SystemData) {
        let mut net_data = match network_data {
            Some(v) => v,
            None => {
                warn!("No network data struct, cannot use networking.");
                return;
            }
        };

        let sender = (&mut net_data).sender.clone();

        // handle incoming packets
        while !self.receiver.is_empty() {
            match self.receiver.try_recv() {
                Ok(v) => match v.message_type {
                    MessageType::ConnectionKeepAlive => {
                        if net_data.is_server {
                            let player_maybe = net_data.player_list.get_mut(&v.client.client_id);
                            match player_maybe {
                                Some(player) => player.last_keep_alive = Instant::now(),
                                None => warn!(
                                    "Got a ConnectionKeepAlive from an unknown client: {:?}",
                                    v.client.client_id
                                ),
                            }
                        } else {
                            net_data.server_last_keep_alive = Instant::now();
                        }
                    }
                    MessageType::ConnectionRequest => {
                        if net_data.is_server {
                            net_data.player_list.insert(
                                v.client.client_id,
                                Player {
                                    client_id: v.client.client_id,
                                    last_keep_alive: Instant::now(),
                                },
                            );
                            match rmp_serde::to_vec(&ConnectionAcceptData {
                                uuid: v.client.client_id,
                            }) {
                                Ok(data) => {
                                    if let Err(e) = net_data.sender.try_send(NetworkPacketOut {
                                        net_id: v.client.client_id,
                                        message_type: MessageType::ConnectionAccept,
                                        protocol: NetworkProtocol::TCP,
                                        data: data,
                                    }) {
                                        error!("Error trying to send ConnectionAccept from ConnectionHandler: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed serializing ConnectionAcceptData: {:?}", e);
                                }
                            }

                            match rmp_serde::to_vec(&NewClientData {
                                uuid: v.client.client_id,
                                name: "Not used yet".into(),
                            }) {
                                Ok(v) => {
                                    for (net_id, _) in net_data.player_list.iter_mut() {
                                        if let Err(e) = sender.try_send(NetworkPacketOut {
                                            net_id: *net_id,
                                            message_type: MessageType::NewClient,
                                            protocol: NetworkProtocol::TCP,
                                            data: v.clone(),
                                        }) {
                                            error!("Could not send NewClientData from ConnectionHandler to {:?}: {:?}", *net_id, e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed serializing NewClientData: {:?}", e);
                                }
                            }
                        } else {
                            warn!("Client somehow got a ConnectionRequest packet????");
                        }
                    }
                    MessageType::ConnectionAccept => {
                        if !net_data.is_server {
                            match rmp_serde::from_slice::<ConnectionAcceptData>(&v.data) {
                                Ok(acc) => {
                                    info!("We got assigned {:?}", acc.uuid);
                                    net_data.player_self = Some(Player {
                                        client_id: acc.uuid,
                                        last_keep_alive: Instant::now(),
                                    });
                                }
                                Err(e) => {
                                    error!(
                                        "Could not parse ConnectionAcceptData on client: {:?}",
                                        e
                                    );
                                }
                            }
                        } else {
                            warn!("Server somehow got a ConnectionAccept packet???");
                        }
                    }
                    _ => {} // we dont care
                },
                Err(e) => error!("Failed receiving net data in ConnectionHandler: {:?}", e),
            }
        }

        // handle outgoing keep alive packets
        if net_data.is_server {
            for (net_id, client) in net_data.player_list.iter_mut() {
                if Instant::now() - client.last_keep_alive >= KEEP_ALIVE_INTERVAL {
                    client.last_keep_alive = Instant::now();
                    if let Err(e) = sender.try_send(NetworkPacketOut {
                        net_id: *net_id,
                        message_type: MessageType::ConnectionKeepAlive,
                        protocol: NetworkProtocol::TCP,
                        ..Default::default()
                    }) {
                        warn!("Could not send server ConnectionKeepAlive from ConnectionHandler to {:?}: {:?}", *net_id, e);
                    }
                }
            }
        } else if let Some(player) = &mut net_data.player_self {
            if Instant::now() - player.last_keep_alive >= KEEP_ALIVE_INTERVAL {
                player.last_keep_alive = Instant::now();
                if let Err(e) = sender.try_send(NetworkPacketOut {
                    net_id: player.client_id,
                    message_type: MessageType::ConnectionKeepAlive,
                    protocol: NetworkProtocol::TCP,
                    ..Default::default()
                }) {
                    warn!(
                        "Could not send client ConnectionKeepAlive from ConnectionHandler: {:?}",
                        e
                    );
                }
            }
        } else if net_data.player_self.is_none() {
            if Instant::now() - net_data.client_connection_tried_last >= KEEP_ALIVE_INTERVAL {
                net_data.client_connection_tried_last = Instant::now();
                if let Err(e) = sender.try_send(NetworkPacketOut {
                    message_type: MessageType::ConnectionRequest,
                    protocol: NetworkProtocol::TCP,
                    ..Default::default()
                }) {
                    warn!(
                        "Could not send client ConnectionRequest from ConnectionHandler: {:?}",
                        e
                    );
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
