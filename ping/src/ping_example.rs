use env_logger;
use log::{debug, warn, info};

use fnv::FnvHashMap;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str,
    time::{Duration, Instant},
};

use futures::{
    prelude::*,
    sync::mpsc::{channel, Sender},
};

use tokio::codec::length_delimited::LengthDelimitedCodec;
use tokio::timer::{Error, Interval};

use p2p::{
    builder::ServiceBuilder,
    service::{Message, ProtocolHandle, ServiceContext, ServiceEvent, ServiceHandle, ServiceTask},
    session::{ProtocolId, ProtocolMeta, SessionId},
    SessionType,
};
use secio::PublicKey;

use ping::{PingNode, Direction, PingStream, PingNodeSubstreamSender};




struct PingProtocol {
    id: usize,
    ty: &'static str,
    notify_counter: u32,
    sessions: HashMap<SessionId, SessionData>,
    inner_task_senders: FnvHashMap<SessionId, Sender<Vec<u8>>>,
    ping_node: Option<PingNode>,
    ping_node_stream_sender: PingNodeSubstreamSender,

}

impl PingProtocol {
    fn new(
        id: usize,
        ty: &'static str,
    ) -> PingProtocol {
        let ping_node = PingNode::new();
        let ping_node_stream_sender = ping_node.get_substream_sender();
        PingProtocol {
            id,
            ty,
            notify_counter: 0,
            sessions: HashMap::default(),
            inner_task_senders: FnvHashMap::default(),
            ping_node: Some(ping_node),
            ping_node_stream_sender,
        }
    }
}

impl ProtocolMeta<LengthDelimitedCodec> for PingProtocol {

    fn id(&self) -> ProtocolId {
        self.id
    }

    fn codec(&self) -> LengthDelimitedCodec {
        LengthDelimitedCodec::new()
    }

    fn handle(&self) -> Option<Box<dyn ProtocolHandle + Send + 'static>> {
        let ping_node = PingNode::new();
        let ping_node_stream_sender = ping_node.get_substream_sender();
        Some(Box::new(PingProtocol {
            id: self.id,
            ty: self.ty,
            notify_counter: 0,
            sessions: HashMap::default(),
            inner_task_senders: FnvHashMap::default(),
            ping_node: Some(ping_node),
            ping_node_stream_sender,
        }))
    }

}

impl ProtocolHandle for PingProtocol {

    fn init(&mut self, control: &mut ServiceContext) {
        debug!("protocol [discovery({})]: init", self.id);

        let mut interval_sender = control.sender().clone();
        let proto_id = self.id();
        let interval_seconds = 5;
        debug!("Setup interval {} seconds", interval_seconds);
        let interval_task = Interval::new(Instant::now(), Duration::from_secs(interval_seconds))
            .for_each(move |_| {
                interval_sender
                .try_send(ServiceTask::ProtocolNotify { proto_id, token: 7 })
                .map_err(|err| {
                    warn!("interval error: {:?}", err);
                    Error::shutdown()
                })
            })
            .map_err(|err| warn!("{}", err));

        debug!("Start ping future_task");
        let ping_task = self.ping_node
            .take()
            .map(|pingnode| {
                debug!("Start ping future_task");
                pingnode
                .for_each(|()| {
                    debug!("ping_node.for_each()");
                    Ok(())
                })
                .map_err(|err| {
                    warn!("ping stream error: {:?}", err);
                    ()
                })
                .then(|_| {
                    warn!("End of ping_task");
                    Ok(())
                })
            }).unwrap();

        control.future_task(interval_task);
        control.future_task(ping_task);

    }

    fn connected(
        &mut self,
        control: &mut ServiceContext,
        session_id: SessionId,
        address: SocketAddr,
        ty: SessionType,
        _: &Option<PublicKey>,
        _: &str,
    ) {
        self.sessions
            .entry(session_id)
            .or_insert(SessionData::new(address, ty));
        debug!(
            "protocol [ping] open on session [{}], address: [{}], type: [{:?}]",
            session_id, address, ty
        );

        let direction = if ty == SessionType::Server {
            Direction::Inbound
        } else {
            Direction::Outbound
        };

        let (sender, receiver) = channel(8);
        self.inner_task_senders.insert(session_id, sender);
        let substream = PingStream::new(
            address,
            direction,
            self.id,
            session_id,
            receiver,
            control.sender().clone(),
        );

        match self.ping_node_stream_sender.substream_sender.try_send(substream) {
            Ok(_) => {
                debug!("Send substream success");
            }
            Err(err) => {
                warn!("Send substream failed : {:?}", err);
            }
        }
    }

    fn disconnected(&mut self, _control: &mut ServiceContext, session_id: SessionId) {
        self.sessions.remove(&session_id);
        self.inner_task_senders.remove(&session_id);
        debug!("protocol [ping] close on session [{}]", session_id);
    }

    fn received(&mut self, _env: &mut ServiceContext, data: Message) {
        debug!("[received message]: length={}", data.data.len());
        self.sessions
            .get_mut(&data.id)
            .unwrap()
            .push_data(data.data.clone());

        if let Some(ref mut sender) = self.inner_task_senders.get_mut(&data.id) {
            if let Err(err) = sender.try_send(data.data) {
                if err.is_full() {
                    warn!("channel is full");
                } else if err.is_disconnected() {
                    warn!("channel is disconnected");
                } else {
                    warn!("other channel error: {:?}", err);
                }
            }
        }
    }

    fn notify(&mut self, _control: &mut ServiceContext, token: u64) {
        debug!("protocol [ping] received notify token: {}", token);
        self.notify_counter += 1;
    }

}

struct SHandle {}

impl ServiceHandle for SHandle {
    fn handle_error(&mut self, _env: &mut ServiceContext, error: ServiceEvent) {
        debug!("service error: {:?}", error);
    }

    fn handle_event(&mut self, _env: &mut ServiceContext, event: ServiceEvent) {
        debug!("service event: {:?}", event);
    }
}

#[derive(Clone)]
struct SessionData {
    ty: SessionType,
    address: SocketAddr,
    data: Vec<Vec<u8>>,
}

impl SessionData {
    fn new(address: SocketAddr, ty: SessionType) -> Self {
        SessionData {
            address,
            ty,
            data: Vec::new(),
        }
    }

    fn push_data(&mut self, data: Vec<u8>) {
        self.data.push(data);
    }
}


fn main() {
    env_logger::init();
    if std::env::args().nth(1) == Some("server".to_string()) {
        debug!("Starting server ......");
        let protocol = PingProtocol::new(0, "PingServer");
        let mut service = ServiceBuilder::default()
            .insert_protocol(protocol)
            .forever(true)
            .build(SHandle {});
        let _ = service.listen("127.0.0.1:1337".parse().unwrap());
        tokio::run(service.for_each(|_| Ok(())))
    } else {
        debug!("Starting client ......");
        let protocol = PingProtocol::new(0, "PingClient");
        let mut service = ServiceBuilder::default()
            .insert_protocol(protocol)
            .forever(true)
            .build(SHandle {})
            .dial("127.0.0.1:1337".parse().unwrap());
        let _ = service.listen("127.0.0.1:1338".parse().unwrap());
        tokio::run(service.for_each(|_| Ok(())))
    }
}
