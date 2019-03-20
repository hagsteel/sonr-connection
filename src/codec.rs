use std::collections::VecDeque;
use std::io::Read;

use bytes::Bytes;

use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecError {
    MalformedMessage(Bytes),
}

#[derive(Debug)]
pub enum Decoding {
    ConnectionError,
    Blocked,
    Succeeded,
}

pub trait Codec: Default {
    type Message: DeserializeOwned;

    fn decode(&mut self, reader: &mut impl Read) -> Decoding;
    fn drain(&mut self) -> VecDeque<Result<Self::Message, CodecError>>;
    fn encode(val: impl Serialize) -> Bytes;
}
