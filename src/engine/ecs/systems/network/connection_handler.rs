use log::{error, warn};
use specs::{Join, Read, ReadStorage, System};

use crate::ecs::{
    components::{general::Transform, network::NetworkReplicated},
    resources::network::{MessageType, NetworkData, NetworkPacket, NetworkProtocol},
};

// Starts and keeps the connection alive

pub struct ConnectionHandler;

impl<'a> System<'a> for ConnectionHandler {
    type SystemData = (Option<Read<'a, NetworkData>>,);

    fn run(&mut self, (network_data,): Self::SystemData) {
        let net_data = match network_data {
            Some(v) => v,
            None => {
                warn!("No network data struct, cannot use networking.");
                return;
            }
        };

        for (net_rep, &t) in (&network_replicated, &transform).join() {
            if net_rep.net_id.is_nil() {
                error!("Tried to update a network replicated entity with respect to transform, which did not have a valid net_id. Ignoring");
                continue;
            }

            match rmp_serde::to_vec(&t) {
                Ok(v) => {
                    let message = NetworkPacket {
                        net_id: net_rep.net_id,
                        message_type: MessageType::ComponentTransform,
                        data: v,
                        protocol: NetworkProtocol::UDP,
                    };

                    // its fine to not await here for now
                    net_data.sender.send(message);
                }
                Err(e) => error!("Could not serialize transform: {e}"),
            };
        }
    }
}
