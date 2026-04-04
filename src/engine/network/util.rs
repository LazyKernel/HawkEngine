struct NetworkMessagingChannelsServer {}

struct NetworkMessagingChannelsClient {}

#[macro_export]
/// Params:
/// sender: A sender for NetworkPacketOut
/// data: A reference to a serializable data
/// target: NetworkTarget
/// message_type: MessageType
/// protocol: NetworkProtocol
macro_rules! send_or_log_err {
    ($sender:expr, $data:expr, $target:expr, $message_type:expr, $protocol:expr) => {
        match rmp_serde::to_vec($data) {
            Ok(v) => {
                if let Err(e) = $sender.try_send(NetworkPacketOut {
                    target: $target,
                    message_type: $message_type,
                    protocol: $protocol,
                    data: v,
                }) {
                    error!("Could not send: {:?}", e);
                }
            }
            Err(e) => error!("Could not serialize: {:?}", e),
        }
    };
}
