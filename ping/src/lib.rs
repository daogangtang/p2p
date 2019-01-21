




pub struct PingNode {
    
    substreams: FnvHashMap<PingStreamKey, PingEndpoint>,

    substream_sender: Sender<Pingstream>,

    substream_receiver: Receiver<Pingstream>,

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

    pub fn recv_substreams(&mut self) -> {
        loop {
            match self.substream_receiver.poll() {
                Ok(Async::Ready(Some(pingstream))) => {
                    let key = pingstream.key();
                    debug!("Received a substream: key={:?}", key);
                    // create PingEndpoint here
                    if key.direction == Direction::InBound {
                        let listener = PingListener::new(pingstream);
                        self.substreams.insert(key, PingEndpoint::Listener(listener));
                    }
                    else {
                        let dialer = PingDialer::new(pingstream);
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

    pub fn get_substream_sender(&self) -> Sender<PingStream>{
        self.substream_sender.clone()
    }

}

impl Stream for PingNode {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        debug!("Ping.poll()");
        self.recv_substreams()?;

        for (key, substream) in self.substreams.iter_mut() {
            // PingListener
            if key.direction == Direction::InBound {
                // poll listener
                let PingEndpoint(listener) = substream;
                match listener.poll() {
                    Ok(Async::Ready(())) => {},
                    Ok(Async::NotReady) => {},
                    Err(err) => warn!(target: "p2p", "Remote ping substream errored: {:?}", err),
                }
            }
            // PingDialer
            else {
                // poll dialer
                let PingEndpoint(dialer) = substream;
                match dialer.poll() {


                }
            }
        }
    }

}


