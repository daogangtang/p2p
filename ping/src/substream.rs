use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::{BufMut, BytesMut};
use futures::{
    sync::mpsc::{Receiver, Sender},
    Async, AsyncSink, Poll, Sink, Stream,
};
use log::{debug, trace, warn};
use tokio::io::{AsyncRead, AsyncWrite};



pub struct PingStream {
    remote_addr: SocketAddr,
    direction: Direction,
    proto_id: ProtocolId,
    session_id: SessionId,

    data_buf: BytesMut,
    pub(crate) receiver: Receiver<Vec<u8>>,
    pub(crate) sender: Sender<ServiceTask>,
}

impl io::Read for PingStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for _ in 0..10 {
            match self.receiver.poll() {
                Ok(Async::Ready(Some(data))) => {
                    self.data_buf.reserve(data.len());
                    self.data_buf.put(&data);
                }
                Ok(Async::Ready(None)) => {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                Ok(Async::NotReady) => {
                    break;
                }
                Err(_err) => {
                    return Err(io::ErrorKind::BrokenPipe.into());
                }
            }
        }
        let n = std::cmp::min(buf.len(), self.data_buf.len());
        if n == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let b = self.data_buf.split_to(n);
        buf[..n].copy_from_slice(&b);
        Ok(n)
    }
}

impl AsyncRead for PingStream {}

impl io::Write for PingStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let task = ServiceTask::ProtocolMessage {
            ids: Some(vec![self.session_id]),
            message: Message {
                id: self.session_id,
                proto_id: self.proto_id,
                data: buf.to_vec(),
            },
        };
        self.sender
            .try_send(task)
            .map(|()| buf.len())
            .map_err(|err| {
                if err.is_full() {
                    io::ErrorKind::WouldBlock.into()
                } else {
                    io::ErrorKind::BrokenPipe.into()
                }
            })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncWrite for PingStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}

impl PingStream {
    pub fn new(
        remote_addr: SocketAddr,
        direction: Direction,
        proto_id: ProtocolId,
        session_id: SessionId,
        receiver: Receiver<Vec<u8>>,
        sender: Sender<ServiceTask>,
    ) -> PingStream {
        PingStream {
            remote_addr,
            direction,
            proto_id,
            session_id,
            receiver,
            sender,
            data_buf: BytesMut::default(),
        }
    }

    pub fn key(&self) -> PingStreamKey {
        PingStreamKey {
            direction: self.direction,
            session_id: self.session_id,
            proto_id: self.proto_id,
        }
    }
}


#[derive(Eq, PartialEq, Hash, Debug, Clone, Copy)]
pub enum Direction {
    Inbound,
    Outbound,
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct PingStreamKey {
    pub(crate) direction: Direction,
    pub(crate) session_id: SessionId,
    pub(crate) proto_id: ProtocolId,
}


