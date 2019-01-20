




pub struct PingNode {
    
    dialer: PingDialer,

    listener: PingListener,

    substreams: FnvHashMap<PingStreamKey, PingStream>,

    substream_sender: Sender<Pingstream>,

    substream_receiver: Receiver<Pingstream>,

}

impl PingNode {

    pub fn new() -> PingNode {
         let (substream_sender, substream_receiver) = channel(8);
         PingNode {
             dialer: PingDialer::new(),
             listener: PingListener::new(),
             substreams: FnvHashMap::default(),
             substream_sender,
             substream_receiver,
         }
    }

    pub fn recv_substreams(&mut self) -> {
        loop {
            match self.substream_receiver.poll() {
                Ok(Async::Ready(Some(substream))) => {
                    let key = substream.key();
                    debug!("Received a substream: key={:?}", key);
                    // TODO: here, do a init ping to start the process
                    // substream.ping();
                    self.substreams.insert(key, substream);
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
                // poll self.listener
                match self.listener.poll() {
                    Ok(Async::Ready(())) => {},
                    Ok(Async::NotReady) => {},
                    Err(err) => warn!(target: "p2p", "Remote ping substream errored: {:?}", err),
                }
            }
            // PingDialer
            else {
                // poll self.dialer
                match self.dialer.poll() {


                }
            }
        }
    }

}


