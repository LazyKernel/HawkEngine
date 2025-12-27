use log::{error, warn};
use specs::{Join, Read, ReadStorage, System};

use crate::ecs::{
    components::{general::Transform, network::NetworkReplicated},
    resources::network::{MessageType, NetworkData, NetworkPacketOut, NetworkProtocol},
};

/// Handler for generic replicated components
/// Responsible for converting Transform updates to network messages
///

pub struct GenericHandler;

impl<'a> System<'a> for GenericHandler {
    type SystemData = (
        ReadStorage<'a, NetworkReplicated>,
        ReadStorage<'a, Transform>,
        Option<Read<'a, NetworkData>>,
    );

    fn run(&mut self, (network_replicated, transform, network_data): Self::SystemData) {
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
                    let message = NetworkPacketOut {
                        net_id: net_rep.net_id,
                        message_type: MessageType::ComponentTransform,
                        data: v,
                        protocol: NetworkProtocol::UDP,
                        ..Default::default()
                    };

                    // its fine to not await here for now
                    net_data.sender.send(message);
                }
                Err(e) => error!("Could not serialize transform: {e}"),
            };
        }
    }
}
