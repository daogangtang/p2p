




pub struct PingNode {
    
    dialer:,

    listener:   ,

    substream_sender: Sender<Substream>,

    substream_receiver: Receiver<Substream>,

}

impl PingNode {

    pub fn new() {

    }

    fn recv_substreams(&mut self) -> {


    }
}

impl Stream for PingNode {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {

    }

}


