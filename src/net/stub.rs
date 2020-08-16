use std::error::Error;
use std::net::SocketAddr;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use super::{NetError, Message, Client, Server};

pub struct StubServer {
    recvq: Receiver<(Message, SocketAddr)>,
    destq: Sender<(Message, SocketAddr)>,
}

impl StubServer {
    pub fn new() -> (StubServer, Sender<(Message, SocketAddr)>, Receiver<(Message, SocketAddr)>) {
        let (recvq_send, recvq_recv) = channel();
        let (destq_send, destq_recv) = channel();
        let server = StubServer {
            recvq: recvq_recv,
            destq: destq_send,
        };
        (server, recvq_send, destq_recv)
    }
}

impl Server for StubServer {
    type Address = SocketAddr;

    fn send(&self, msg: &Message, addr: &SocketAddr) -> Result<(), NetError> {
        self.destq.send((msg.to_owned(), addr.to_owned()))
            .map_err(|e| NetError::Error(Box::new(e)))
    }

    fn recv(&mut self) -> Result<(Message, SocketAddr), NetError> {
        match self.recvq.try_recv() {
            Err(TryRecvError::Empty) => Err(NetError::NoMore),
            Err(e @ TryRecvError::Disconnected) => Err(NetError::Error(Box::new(e))),
            Ok((msg, addr)) => Ok((msg, addr)),
        }
    }
}

pub struct StubClient<S: Fn(&Message) -> Result<(), Box<dyn Error>> + Send + 'static> {
    recvq: Receiver<Message>,
    sender: S,
}

impl<S: Fn(&Message) -> Result<(), Box<dyn Error>> + Send + 'static> StubClient<S> {
    pub fn new(sender: S) -> (StubClient<S>, Sender<Message>) {
        let (recvq_send, recvq_recv) = channel();
        let client = StubClient {
            recvq: recvq_recv,
            sender,
        };
        (client, recvq_send)
    }
}

impl<S: Fn(&Message) -> Result<(), Box<dyn Error>> + Send + 'static> Client for StubClient<S> {
    fn send(&self, msg: &Message) -> Result<(), NetError> {
        (self.sender)(msg).map_err(|e| NetError::Error(e))
    }

    fn recv(&mut self) -> Result<Message, NetError> {
        match self.recvq.try_recv() {
            Err(TryRecvError::Empty) => Err(NetError::NoMore),
            Err(e @ TryRecvError::Disconnected) => Err(NetError::Error(Box::new(e))),
            Ok(msg) => Ok(msg),
        }
    }
}
