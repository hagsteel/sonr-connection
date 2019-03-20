use std::collections::VecDeque;
use std::io::{ErrorKind::WouldBlock, Read, Write};

use sonr::net::stream::{Stream, StreamRef};
use sonr::reactor::{Reaction, Reactor};
use sonr::Token;

use bytes::Bytes;

use crate::codec::{Codec, CodecError};

pub enum ConnectionError {
    ConnectionClosed,
}

impl From<ConnectionError> for Result<Bytes, ConnectionError> {
    fn from(e: ConnectionError) -> Result<Bytes, ConnectionError> {
        Err(e)
    }
}

pub struct Connection<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    inner: T,
    write_buffers: VecDeque<Bytes>,
    message_buffer: VecDeque<Result<C::Message, CodecError>>,
    codec: C,
}

impl<T, C> Connection<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    pub fn new(stream: T, codec: C) -> Self {
        Self {
            inner: stream,
            write_buffers: VecDeque::new(),
            message_buffer: VecDeque::new(),
            codec,
        }
    }

    pub fn stream_ref(&self) -> &Stream<T::Evented> {
        self.inner.stream_ref()
    }

    pub fn stream_mut(&mut self) -> &mut Stream<T::Evented> {
        self.inner.stream_mut()
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn token(&self) -> Token {
        self.inner.stream_ref().token()
    }

    pub fn add_write_buffer(&mut self, buffer: Bytes) {
        self.write_buffers.push_back(buffer);
    }

    pub fn write_buffers(&mut self) {
        while self.inner.stream_ref().writable() && !self.write_buffers.is_empty() {
            if let Some(buf) = self.write_buffers.pop_front() {
                match self.inner.write(&buf) {
                    Ok(n) => {
                        if n != buf.len() {
                            let remainder = Bytes::from(&buf[n..]);
                            self.write_buffers.push_front(remainder);
                        }
                    }
                    Err(ref e) if e.kind() == WouldBlock => {
                        self.write_buffers.push_front(buf);
                    }
                    Err(ref _e) => { }//return Reaction::Value(ConnectionError::ConnectionClosed.into()),
                }
            }
        }
    }

    fn read_and_decode(&mut self) {
        if self.inner.stream_ref().readable() {
            self.codec.decode(&mut self.inner);
            let mut messages = self.codec.drain();
            self.message_buffer.append(&mut messages);
        }
    }
}

impl<T, C> Reactor for Connection<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    type Input = Bytes;
    type Output = Result<C::Message, CodecError>;

    fn react(&mut self, reaction: Reaction<Self::Input>) -> Reaction<Self::Output> {
        match reaction {
            Reaction::Value(bytes) => {
                self.add_write_buffer(bytes);
                self.write_buffers();
                Reaction::Continue
            }

            Reaction::Event(ev) => {
                if ev.token() != self.inner.stream_ref().token() {
                    return ev.into();
                }
                self.inner.stream_mut().react(ev.into());

                self.write_buffers();
                self.read_and_decode();

                if let Some(msg) = self.message_buffer.pop_front() {
                    Reaction::Value(msg)
                } else {
                    Reaction::Continue
                }
            }

            Reaction::Continue => {
                self.read_and_decode();

                if let Some(msg) = self.message_buffer.pop_front() {
                    Reaction::Value(msg)
                } else {
                    Reaction::Continue
                }
            }
        }
    }
}
