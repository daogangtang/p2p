

pub struct PingDialer {

    inner: Framed<PingStream, Codec>,

    sent_pings: VecDeque<(Bytes, TUserData)>,

    rng: EntropyRng,

    pings_to_send: VecDeque<(Bytes, TUserData)>,

    state: PingDialerState,

    need_writer_flush: bool,
    needs_close: bool,

    ping_timeout: Duration,

    delay_to_next_ping: Duration,


}

enum PingDialerState {
    WaitingForPong {
        expires: Delay,   
    },
    Idle {
        next_ping: Delay,   
    },
    Shutdown,
    Poisoned,
}

#[derive(Debug, Copy, Clone)]
pub enum OutState {
    PingStart,
    PingSuccess(Duration),
    Shutdown
}



impl PingDialer {
    pub fn new(pingstream: PingStream) -> PingDialer {
        let ping_timeout = Duration::from_secs(30);

        PingDialer {
            inner: Framed::new(pingstream, Codec),
            sent_pings: VecDeque::with_capacity(4),
            rng: EntropyRng::default(),
            pings_to_send: VecDeque::with_capacity(4),
            state: PingDialerState::Idle {next_ping: Delay::new(Instant::now() + ping_timeout)},
            need_writer_flush: false,
            needs_close: false,

            ping_timeout: ping_timeout,
            delay_to_next_ping: Duration::from_secs(15),
        }

    }

    pub fn ping(&self) {
        let payload: [u8; 32] = self.rng.sample(Standard);
        debug!("Preparing for ping with payload {:?}", payload);
        self.pings_to_send.push_back((Bytes::from(payload.to_vec()), user_data));
    }

    #[inline]
    pub fn shutdown(&mut self) {
        self.needs_close = true;
    }

    pub fn send_pings() -> Result<(), io::Error> {
        //TODO: divide this part as sending pings part
        while let Some((ping, user_data)) = self.pings_to_send.pop_front() {
            match self.inner.start_send(ping.clone()) {
                Ok(AsyncSink::Ready) => self.need_writer_flush = true,
                    Ok(AsyncSink::NotReady(_)) => {
                        self.pings_to_send.push_front((ping, user_data));
                        break;
                    },
                    Err(err) => return Err(err),
            }

            self.sent_pings.push_back((ping, user_data));
        }
        
        if self.need_writer_flush {
            match self.inner.poll_complete() {
                Ok(Async::Ready(())) => self.need_writer_flush = false,
                Ok(Async::NotReady) => (),
                Err(err) => return Err(err),
            }
        }

        Ok(())

    }

    fn receive_pings(&mut self) -> Poll<Option<()>, io::Error> {
        //TODO: divide this part as recieving pings part
        loop {
            match self.inner.poll() {
                Ok(Async::Ready(Some(pong))) => {
                    if let Some(pos) = self.sent_pings.iter().position(|&(ref p, _)| p == &pong) {
                        let (_, user_data) = self.sent_pings.remove(pos)
                            .expect("Grabbed a valid position just above");
                        return Ok(Async::Ready(Some(user_data)));
                    } else {
                        debug!("Received pong that doesn't match what we sent: {:?}", pong);
                    }
                },
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(None)) => {
                    self.needs_close = true;
                    try_ready!(self.inner.close());
                    return Ok(Async::Ready(None));
                }
                Err(err) => return Err(err),
            }
        }

        Ok(Async::NotReady)
    }
}

impl Stream for PingDialer {
    type Item = OutState;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {

        macro_rules! poll_delay {
            ($delay:expr => { NotReady => $notready:expr, Ready => $ready:expr, }) => (
                match $delay.poll() {
                    Ok(Async::NotReady) => $notready,
                Ok(Async::Ready(())) => $ready,
                Err(err) => {
                    warn!(target: "p2p", "Ping timer errored: {:?}", err);
                    return Err(io::Error::new(io::ErrorKind::Other, err));
                }
                }
            )
        }

        if self.needs_close {
            try_ready!(self.inner.close());
            return Ok(Async::Ready(None));
        }

        // here, add state checking logic, and calling sending part and recieving part
        match mem::replace(&mut self.out_state, PingDialerState::Poisoned) {

            PingDialerState::WaitingForPong {mut expires} => {

                match self.send_pings() {
                    Ok(_) => {

                    },
                    Err(_) => {

                    }
                }

                match self.receive_pings()? {
                     Async::Ready(Some(started)) => {
                         self.state = PingDialerState::Idle {
                            next_ping:  Delay::new(Instant::now() + self.delay_to_next_ping)
                         };

                         return Ok(Async::Ready(OutState::PingSuccess(started.elapsed())));
                     },
                     Async::NotReady => {},
                     Async::Ready(None) => {
                         self.state = PingDialerState::Shutdown;
                         return Ok(Async::Ready(OutState::Shutdown));
                     }
                }

                // Check the expiration
                poll_delay!(expires => {
                    NotReady => {
                        self.state = PingDialerState::WaitingForPong {expires};
                        Ok(Async::NotReady)
                    },
                    Ready => {
                        self.state = PingDialerState::Shutdown;
                        Err(io::Error::new(io::ErrorKind::Other, "unresponsive node"))
                    },
                })

            },

            PingDialerState::Idle {mut next_ping} => {
                poll_delay!(next_ping => {
                    NotReady => {
                        self.state = PingDialerState::Idle {next_ping};
                        Ok(Async::NotReady)
                    },
                    Ready => {
                        let expires = Delay::new(Instant::now() + self.ping_timeout);
                        self.ping(Instant::now());
                        self.state = PingDialerState::WaitingForPong { expires };
                        Ok(Async::Ready(OutState::PingStart)))
                    },
                })

            }

        }

        Ok(Async::NotReady)
    }

}


pub struct PingListener {

    inner: Framed<PingStream, Codec>,

    state: PingListenerState,
}

enum PingListenerState {
    Listening,
    Sending(Bytes),
    Flushing,
    CLosing,
    Poisoned,
}

impl PingListener {
    pub fn new(pingstream: PingStream) -> PingListener {
        PingListener {
            inner: Framed::new(pingstream, Codec),
            state: PingListenerState::Listening
        }
    }

    pub fn shutdown(&mut self) {
        self.state = PingListenerState::Closing;
    }
}

impl Future for PingListener {
    type Item = ();
    type Error = IoError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match mem::replace(&mut self.state, PingListenerState::Poisoned) {
                PingListenerState::Listening => {
                    match self.inner.poll() {
                        Ok(Async::Ready(Some(payload))) => {
                            debug!("Received ping (payload={:?}); sending back", payload);
                            self.state = PingListenerState::Sending(payload.freeze())
                        },
                        Ok(Async::Ready(None)) => self.state = PingListenerState::Closing,
                        Ok(Async::NotReady) => {
                            self.state = PingListenerState::Listening;
                            return Ok(Async::NotReady);
                        },
                        Err(err) => return Err(err),
                    }
                },
                PingListenerState::Sending(data) => {
                    match self.inner.start_send(data) {
                        Ok(AsyncSink::Ready) => self.state = PingListenerState::Flushing,
                        Ok(AsyncSink::NotReady(data)) => {
                            self.state = PingListenerState::Sending(data);
                            return Ok(Async::NotReady);
                        },
                        Err(err) => return Err(err),
                    }
                },
                PingListenerState::Flushing => {
                    match self.inner.poll_complete() {
                        Ok(Async::Ready(())) => self.state = PingListenerState::Listening,
                        Ok(Async::NotReady) => {
                            self.state = PingListenerState::Flushing;
                            return Ok(Async::NotReady);
                        },
                        Err(err) => return Err(err),
                    }
                },
                PingListenerState::Closing => {
                    match self.inner.close() {
                        Ok(Async::Ready(())) => return Ok(Async::Ready(())),
                        Ok(Async::NotReady) => {
                            self.state = PingListenerState::Closing;
                            return Ok(Async::NotReady);
                        },
                        Err(err) => return Err(err),
                    }
                },
                PingListenerState::Poisoned => panic!("Poisoned or errored PingListener"),
            }
        }
    }

}



struct Codec;
impl Decoder for Codec {
    type Item = BytesMut;
    type Error = IoError;

    #[inline]
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, IoError> {
        if buf.len() >= 32 {
            Ok(Some(buf.split_to(32)))
        } else {
            Ok(None)
        }
    }
}

impl Encoder for Codec {
    type Item = Bytes;
    type Error = IoError;

    #[inline]
    fn encode(&mut self, mut data: Bytes, buf: &mut BytesMut) -> Result<(), IoError> {
        if !data.is_empty() {
            let split = 32 * (1 + ((data.len() - 1) / 32));
            buf.reserve(split);
            buf.put(data.split_to(split));
        }
        Ok(())
    }
}

pub enum PingEndpoint {
    Dialer(PingDialer),
    Listener(PingListener)
}




#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
