use std::io::{Read, Write};
use crate::data_types::var_int::VarInt;
use std::io::Result;

pub struct ProtocolString {
    internal: String
}

impl ProtocolString {
    pub fn from_str(s: &str) -> ProtocolString {
        ProtocolString { internal: String::from(s) }
    }

    pub fn from_stream(s: &mut dyn Read) -> ProtocolString {
        let size = VarInt::from_stream(s);
        // let mut buf: Vec<u8> = Vec::with_capacity(size.int_val as usize);
        let mut buf = vec![0; *size.int_val() as usize];
        s.read_exact(buf.as_mut_slice()).expect("Error reading ProtocolString from stream");
        let str_val = String::from_utf8(buf)
            .expect("Decoded ProtocolString is not a valid UTF-8 string.");

        ProtocolString { internal: str_val }
    }

    pub fn write_to_stream(&self, s: &mut dyn Write) -> Result<()> {
        VarInt::from_int(self.internal.len() as u64).write_to_stream(s)?;
        s.write_all(self.internal.as_bytes())?;
        Ok(())
    }

    pub fn to_str(&self) -> &str {
        self.internal.as_str()
    }
}