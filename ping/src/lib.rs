use std::collections::VecDeque;
use std::io;

use fnv::{FnvHashMap, FnvHashSet};
use futures::{
    Future,
    sync::mpsc::{channel, Receiver, Sender},
    Async, Poll, Stream,
};
use log::{debug, warn};

mod substream;
mod protocol;

pub use crate::{
    substream::{PingStream, Direction, PingStreamKey},
    protocol::{PingDialer, PingListener, PingEndpoint},
};


pub struct PingNode {
    
    substreams: FnvHashMap<PingStreamKey, PingEndpoint>,

    substream_sender: Sender<PingStream>,

    substream_receiver: Receiver<PingStream>,

}

#[derive(Clone)]
pub struct PingNodeSubstreamSender {
    pub substream_sender: Sender<PingStream>,
}


impl PingNode {

    pub fn new() -> PingNode {
         let (substream_sender, substream_receiver) = channel(8);
         PingNode {
             substreams: FnvHashMap::default(),
             substream_sender,
             substream_receiver,
         }
    }

    pub fn recv_substreams(&mut self) -> Result<(), io::Error> {
        loop {
            match self.substream_receiver.poll() {
                Ok(Async::Ready(Some(pingstream))) => {
                    let key = pingstream.key();
                    debug!("Received a substream: key={:?}", key);
                    // create PingEndpoint here
                    if key.direction == Direction::Inbound {
                        let listener = PingListener::new(pingstream);
                        self.substreams.insert(key, PingEndpoint::Listener(listener));
                    }
                    else {
                        let mut dialer = PingDialer::new(pingstream);
                        // do a ping action once PingDialer created
                        dialer.ping();
                        self.substreams.insert(key, PingEndpoint::Dialer(dialer));
                    }

                },
                Ok(Async::Ready(None)) => unreachable!(),
                Ok(Async::NotReady) => {
                    debug!("Discovery.substream_receiver Async::NotReady");
                    break;
                },
                Err(err) => {
                    debug!("receive substream error: {:?}", err);
                    return Err(io::ErrorKind::Other.into());
                }
            }
        }

        Ok(())
    }

    pub fn get_substream_sender(&self) -> PingNodeSubstreamSender {
        PingNodeSubstreamSender {
            substream_sender: self.substream_sender.clone()
        }
    }

}

impl Stream for PingNode {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        debug!("Ping.poll()");
        self.recv_substreams()?;

        for (key, substream) in self.substreams.iter_mut() {
            match substream {
                // poll listener
                PingEndpoint::Listener(listener) => {
                    match listener.poll() {
                        Ok(Async::Ready(_)) => {},
                        Ok(Async::NotReady) => {},
                        Err(err) => warn!(target: "p2p", "Remote ping substream errored: {:?}", err),
                    }
                },
                PingEndpoint::Dialer(dialer) => {
                    match dialer.poll() {
                        Ok(Async::Ready(Some(_))) => {},
                        Ok(Async::Ready(None)) => {},
                        Ok(Async::NotReady) => {},
                        Err(err) => warn!(target: "p2p", "ping substream errored: {:?}", err),
                    }
                }
            }
        }

        Ok(Async::NotReady)
    }

}


