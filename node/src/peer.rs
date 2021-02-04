use crate::cookie::Cookie;
use crate::header::{Flags, Header, MessageType};
use crate::messages::node_id_handshake::{NodeIdHandshakeQuery, NodeIdHandshakeResponse};
use crate::state::State;
use crate::wire::Wire;
use anyhow::anyhow;
use feeless::Seed;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct Peer {
    state: State,
    stream: TcpStream,
    peer_addr: SocketAddr,

    /// A reusable header to reduce allocations.
    header: Header,

    /// Storage that can be shared within this task without reallocating.
    buffer: Vec<u8>,
}

impl Peer {
    pub fn new(state: State, stream: TcpStream) -> Self {
        let network = state.network();
        // TODO: Remove unwrap
        let peer_addr = stream.peer_addr().unwrap();
        Self {
            state,
            stream,
            peer_addr,
            header: Header::new(network, MessageType::NodeIdHandshake, Flags::new()),
            buffer: Vec::with_capacity(1024),
        }
    }

    async fn recv<T: Wire>(&mut self) -> anyhow::Result<T> {
        let len = T::len();

        if len > self.buffer.len() {
            self.buffer.resize(len, 0)
        }

        let buffer = &mut self.buffer[0..len];
        let bytes_read = self.stream.read_exact(buffer).await?;
        if bytes_read < len {
            return Err(anyhow!(
                "Received an incorrect amount of bytes. Got: {} Expected: {}",
                bytes_read,
                len,
            ));
        }

        let buffer = &self.buffer[0..len];
        Ok(T::deserialize(&self.state, buffer)?)
    }

    async fn send<T: Wire>(&mut self, message: &T) -> anyhow::Result<()> {
        self.stream.write_all(&message.serialize()).await?;
        Ok(())
    }

    pub async fn send_header(
        &mut self,
        message_type: MessageType,
        flags: Flags,
    ) -> anyhow::Result<()> {
        let mut header = self.header;
        header.reset(message_type, flags);
        Ok(self.send(&header).await?)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.initial_handshake().await?;

        loop {
            let header = self.recv::<Header>().await?;
            dbg!(&header);

            match header.message_type() {
                MessageType::Keepalive => todo!(),
                MessageType::Publish => todo!(),
                MessageType::ConfirmReq => todo!(),
                MessageType::ConfirmAck => todo!(),
                MessageType::BulkPull => todo!(),
                MessageType::BulkPush => todo!(),
                MessageType::FrontierReq => todo!(),
                MessageType::NodeIdHandshake => self.handle_node_id_handshake(header).await?,
                MessageType::BulkPullAccount => todo!(),
                MessageType::TelemetryReq => todo!(),
                MessageType::TelemetryAck => todo!(),
            }
        }
    }

    async fn handle_node_id_handshake(&mut self, header: Header) -> anyhow::Result<()> {
        if header.flags().is_query() {
            let query = self.recv::<NodeIdHandshakeQuery>().await?;
            // XXX: Hacky code here just to see if it works!
            let seed = Seed::random();
            let private = seed.derive(0);
            let public = private.to_public();
            let signature = private.sign(query.cookie().as_bytes())?;

            debug_assert!(public.verify(query.cookie().as_bytes(), &signature));

            let mut header = self.header;
            header.reset(MessageType::NodeIdHandshake, *Flags::new().response(true));
            self.send(&header).await?;

            let response = NodeIdHandshakeResponse::new(public, signature);
            dbg!("sending handshake response");
            self.send(&response).await?;
            dbg!("sending handshake response done");
        }
        if header.flags().is_response() {
            let response = self.recv::<NodeIdHandshakeResponse>().await?;
            let public = response.public;
            let signature = response.signature;

            let cookie = &self.state.cookie_for_socket_addr(&self.peer_addr).await?;

            if !public.verify(&cookie.as_bytes(), &signature) {
                return Err(anyhow!("Invalid signature in node_id_handshake response"));
            }
            dbg!("signature verified");
        }
        Ok(())
    }

    async fn initial_handshake(&mut self) -> anyhow::Result<()> {
        self.send_header(MessageType::NodeIdHandshake, *Flags::new().query(true))
            .await?;

        let cookie = Cookie::random();
        self.state
            .set_cookie(self.peer_addr, cookie.clone())
            .await?;
        let handshake_query = NodeIdHandshakeQuery::new(cookie);
        dbg!("sending cookie");
        self.send(&handshake_query).await?;

        Ok(())
    }
}
