use std::sync::mpsc::{Sender, Receiver};


pub struct NetworkMessageData {
    data: u8
}

pub struct NetworkData {
    pub sender: Sender<NetworkMessageData>,
    pub receiver: Receiver<NetworkMessageData>
}
