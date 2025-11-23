struct NetworkMessagingChannelsServer {

    let (tokio_to_game_sender, mut tokio_to_game_receiver) = mpsc::channel::<NetworkMessage>(16384);
    let (game_to_tokio_sender, game_to_tokio_receiver) = broadcast::channel::<NetworkMessage>(16384);

    let (tokio_to_game_sender_udp, mut tokio_to_game_receiver_udp) = mpsc::channel::<NetworkMessage>(16384);
    let (game_to_tokio_sender_udp, game_to_tokio_receiver_udp) = mpsc::channel::<NetworkMessage>(16384);

}

struct NetworkMessagingChannelsClient {

}
