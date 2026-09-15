use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::io;

pub const MAX_FRAME: usize = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    version: u16,
    message: T,
}

pub fn encode<T: Serialize>(message: &T) -> io::Result<Vec<u8>> {
    let payload = serde_json::to_vec(&Envelope {
        version: 1,
        message,
    })
    .map_err(invalid)?;
    if payload.len() > MAX_FRAME {
        return Err(invalid("message exceeds frame limit"));
    }
    let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
    bytes.extend(payload);
    Ok(bytes)
}

#[derive(Default)]
pub struct Decoder {
    bytes: Vec<u8>,
    failed: bool,
}

impl Decoder {
    pub fn feed<T: DeserializeOwned>(&mut self, bytes: &[u8]) -> io::Result<Vec<T>> {
        if self.failed {
            return Err(invalid("connection already rejected"));
        }
        let result = self.decode(bytes);
        if result.is_err() {
            self.failed = true;
            self.bytes.clear();
        }
        result
    }

    fn decode<T: DeserializeOwned>(&mut self, mut input: &[u8]) -> io::Result<Vec<T>> {
        let mut messages = Vec::new();
        while !input.is_empty() {
            if self.bytes.len() < 4 {
                let count = (4 - self.bytes.len()).min(input.len());
                self.bytes.extend_from_slice(&input[..count]);
                input = &input[count..];
                if self.bytes.len() < 4 {
                    break;
                }
            }
            let size = u32::from_be_bytes(self.bytes[..4].try_into().unwrap()) as usize;
            if size == 0 || size > MAX_FRAME {
                return Err(invalid("invalid frame length"));
            }
            let count = (size + 4 - self.bytes.len()).min(input.len());
            self.bytes.extend_from_slice(&input[..count]);
            input = &input[count..];
            if self.bytes.len() == size + 4 {
                let envelope: Envelope<T> =
                    serde_json::from_slice(&self.bytes[4..]).map_err(invalid)?;
                if envelope.version != 1 {
                    return Err(invalid("unsupported protocol version"));
                }
                messages.push(envelope.message);
                self.bytes.clear();
            }
        }
        Ok(messages)
    }

    pub fn finish(&self) -> io::Result<()> {
        if self.bytes.is_empty() && !self.failed {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete or rejected frame",
            ))
        }
    }
}

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

#[cfg(test)]
mod tests;
