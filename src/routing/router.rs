use std::{sync::{Arc, Mutex, mpsc::{RecvTimeoutError, Receiver, Sender, self, RecvError}}, time::Duration};

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

pub trait Handler {
    type T: MessageContent;
    fn process(&self, msg: &Message<Self::T>) -> Message<Self::T>;
}

pub struct Router<H: Handler> {
    tx_in: Sender<Message<<H as Handler>::T>>,
    rx_in: Receiver<Message<<H as Handler>::T>>,

    tx_out: Sender<Message<<H as Handler>::T>>,
    rx_out: Receiver<Message<<H as Handler>::T>>,

    bus: Bus<(MessageType, Message<<H as Handler>::T>)>,
    handler: H,
}

impl< H: Handler> Router<H> {
    pub fn new(handler: H) -> Router<H> {
        let (tx_in, rx_in) = mpsc::channel();
        let (tx_out, rx_out) = mpsc::channel();
        let bus = Bus::new();
        Router { tx_in, rx_in, tx_out, rx_out, bus, handler }
    }

    // pub fn bus(&self) -> &Bus<(MessageType, Message<T>)> {
    //     &self.bus
    // }

    pub fn tx(&self) -> Sender<Message<<H as Handler>::T>> {
        self.tx_in.clone()
    }

    pub fn recv(&self) -> Result<Message<<H as Handler>::T>, RecvError> {
        match self.rx_in.recv() {
            Ok(msg) => {
                let result = self.handler.process(&msg);
                Ok(result)
            },
            Err(err) => Err(err),
        }
    }

    pub fn recv_timeout(&self, duration: Duration) -> Result<Message<<H as Handler>::T>, RecvTimeoutError> {
        match self.rx_in.recv_timeout(duration) {
            Ok(msg) => {
                let result = self.handler.process(&msg);
                Ok(result)
            },
            Err(err) => Err(err),
        }
    }
}

mod tests {
    use std::{sync::{atomic::AtomicU32, Arc}, thread, time::Duration};
    use crate::message::format::{MessageContent, Message, Header, MessageType, Empty};

    use super::{Bus, Handler, Router};

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

    struct DummyHandler {}

    impl Handler for DummyHandler {
        type T = Empty;
        
        fn process(&self,
            msg: &Message<Self::T>
        ) -> Message<Self::T> {
            let mut count = 0;
            match msg.header().msg_type() {
                MessageType::Alive => count += 1,
                MessageType::MemberReq => count += 2,
                MessageType::MemberRes => count += 3,
                MessageType::Chat => count += 4,
            };

            Message::<Empty>::new(
                Header::new(1, msg.header().msg_type(), count.try_into().unwrap()),
                Empty::new())
        }
    }

    #[test]
    fn router() {
        let MSGS: [Message::<Empty>; 5] = [
            Message::<Empty>::new(Header::new(1, MessageType::Alive, 5), Empty::new()),
            Message::<Empty>::new(Header::new(1, MessageType::MemberReq, 15), Empty::new()),
            Message::<Empty>::new(Header::new(1, MessageType::MemberRes, 77), Empty::new()),
            Message::<Empty>::new(Header::new(1, MessageType::Chat, 343), Empty::new()),
            Message::<Empty>::new(Header::new(1, MessageType::Alive, 5), Empty::new()),
        ];

        let mut router = Router::new(DummyHandler{});

        let main_sender = router.tx();

        let t = thread::spawn(move || {
            let mut count = 0;
            loop {
                match router.recv_timeout(Duration::from_secs(2)) {
                    Ok(msg) => {
                        match msg.header().msg_type() {
                            MessageType::Alive => {
                                assert_eq!(msg.header().msg_size(), 1);
                                count += msg.header().msg_size();
                            },
                            MessageType::MemberReq => {
                                assert_eq!(msg.header().msg_size(), 2);
                                count += msg.header().msg_size();
                            },
                            MessageType::MemberRes => {
                                assert_eq!(msg.header().msg_size(), 3);
                                count += msg.header().msg_size();
                            },
                            MessageType::Chat => {
                                assert_eq!(msg.header().msg_size(), 4);
                                count += msg.header().msg_size();
                            },
                        }
                        
                    },
                    Err(err) => break,
                }
            }
            assert_eq!(count, 11);
        });

        let mut sender_handles = Vec::new();
        for msg in MSGS {
            let sender = main_sender.clone();
            sender_handles.push(
                thread::spawn(move || {
                    sender.send(msg).unwrap();
                })
            );
        }

        for handle in sender_handles {
            handle.join().unwrap();
        }

        drop(main_sender);

        t.join().unwrap();
    }
}