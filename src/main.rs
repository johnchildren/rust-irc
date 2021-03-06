extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

extern crate bytes;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rmp_serde as rmps;

use std::io;
use std::str;

use tokio_io::{AsyncRead};
use tokio_io::codec::{Encoder, Decoder};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_service::{Service, NewService};
use futures::{future, Future, BoxFuture, Stream, Sink};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use rmps::{Deserializer, Serializer};

#[derive(Deserialize, Serialize)]
pub struct Message {
    id: u32,
    body: String,
}

pub struct MessageCodec;

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Message>> {
        if let Some(i) = buf.iter().position(|&b| b == b'\n') {
            let line = buf.split_to(i);

            buf.split_to(1);

            let mut de = Deserializer::new(&line[..]);
            match Deserialize::deserialize(&mut de) {
                Ok(s) => Ok(Some(s)),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid message")),
            }
        } else {
            Ok(None)
        }
    }
}

impl Encoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, msg: Message, buf: &mut BytesMut) -> io::Result<()> {
        let mut bytes = Vec::new();
        msg.serialize(&mut Serializer::new(&mut bytes)).unwrap();
        buf.extend(bytes);
        buf.extend(b"\n");
        Ok(())
    }
}

fn serve<S>(s: S) -> io::Result<()>
    where S: NewService<Request = Message, Response = Message, Error = io::Error> + 'static
{
    let mut core = Core::new()?;
    let handle = core.handle();

    let address = "0.0.0.0:12345".parse().unwrap();
    let listener = TcpListener::bind(&address, &handle)?;

    let connections = listener.incoming();
    let server = connections.for_each(move |(socket, _peer_addr)| {
        let (writer, reader) = socket.framed(MessageCodec).split();
        let service = s.new_service()?;

        let responses = reader.and_then(move |req| service.call(req));
        let server = writer.send_all(responses)
            .then(|_| Ok(()));
        handle.spawn(server);

        Ok(())
    });

    core.run(server)
}

struct EchoService;

impl Service for EchoService {
    type Request = Message;
    type Response = Message;
    type Error = io::Error;
    type Future = BoxFuture<Message, io::Error>;

    fn call(&self, input: Message) -> Self::Future {
        future::ok(input).boxed()
    }
}

fn main() {
    if let Err(e) = serve(|| Ok(EchoService)) {
        println!("Server failed for{}", e)
    }
}
