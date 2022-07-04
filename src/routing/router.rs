use std::sync::{Arc, Mutex, mpsc::{RecvTimeoutError, Receiver, Sender, self}};

use multiqueue::{BroadcastSender, BroadcastReceiver};

use crate::message::format::{Message, MessageContent, MessageType};

pub struct Bus<T: Clone> {
    // TODO: remove hard deps on multiqueue package; encapsulate Broadcast{Sender,Receiver}
    sender: BroadcastSender<T>,
    receiver: BroadcastReceiver<T>,
}

impl<T: Clone> Bus<T> {
    pub fn new() -> Bus<T> {
        let (tx, rx) = multiqueue::broadcast_queue(256);
        Bus { sender: tx, receiver: rx }
    }

    pub fn rx(&self) -> BroadcastReceiver<T> {
        self.receiver.add_stream()
    }

    pub fn tx(&self) -> BroadcastSender<T> {
        self.sender.clone()
    }
}

pub trait Handler<T: MessageContent> {
    fn process(&self, msg_type: MessageType, msg: &T) -> T;
}

pub struct Router<T: MessageContent, H: Handler<T>> {
    tx: Sender<T>,
    rx: Receiver<T>,

    bus: Bus<(MessageType, Message<T>)>,
    handlers: Arc<Mutex<Vec<H>>>,
}

impl<T: MessageContent, H: Handler<T>> Router<T, H> {
    pub fn new() -> Router<T, H> {
        let (tx, rx) = mpsc::channel();
        let bus = Bus::new();
        let handlers = Arc::new(Mutex::new(Vec::new()));
        Router { tx, rx, bus, handlers }
    }

    pub fn bus(&self) -> &Bus<(MessageType, Message<T>)> {
        &self.bus
    }

    pub fn register_handler(&mut self, handler: H) {
        self.handlers.lock().unwrap().push(handler);
    }

    pub fn execute_handlers(&self, msg_type: MessageType, msg: Message<T>) {
        let handlers = self.handlers.lock().unwrap();
        for handler in handlers.iter() {
            let result = handler.process(msg_type, msg.content());
            self.tx.send(result).unwrap();
        }
    } 
}

mod tests {
    use std::sync::{atomic::AtomicU32, Arc};
    use super::Bus;

    #[test]
    fn bus_broadcast() {
        let bus: Bus<u32> = Bus::new();

        let counter = Arc::new(AtomicU32::new(0));
        let mut handles = Vec::new();

        for i in 0..10 {
            let recv = bus.rx();
            let t_counter = counter.clone();
            handles.push(
                std::thread::spawn(move || {
                    for val in recv {
                        t_counter.fetch_add(val, std::sync::atomic::Ordering::Acquire);
                    }
                })
            );
        }

        for i in 0..10 {
            bus.tx().try_send(i).unwrap();
        }

        drop(bus);

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 450);
    }
}