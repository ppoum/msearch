// A lot of info taken from https://wiki.vg/Protocol

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{Read, Write};

#[derive(Debug)]
pub struct PacketParseError(pub String);

impl PacketParseError {
    pub fn message(&self) -> String {
        self.0.clone()
    }
}

impl PacketParseError {
    pub fn new(s: &str) -> Self {
        Self(String::from(s))
    }
}

impl Display for PacketParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error parsing data: {}", self.0)
    }
}

impl Error for PacketParseError {}

pub struct MCPacket {
    id: Vec<u8>,
    data: Vec<u8>
}

impl MCPacket {
    pub fn new(id: u64) -> Self {
        MCPacket {
            id: MCPacket::int_to_var_int(id),
            data: Vec::new()
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let packet_length = MCPacket::int_to_var_int((self.id.len() + self.data.len()) as u64);
        let mut result = packet_length;
        result.append(&mut self.id.clone());
        if !self.data.is_empty() { result.append(&mut self.data.clone()); }
        result
    }

    pub fn write_to_stream(&self, s: &mut dyn Write) {
        s.write_all(self.to_bytes().as_ref()).expect("Error writing to stream");
    }

    pub fn write_var_int(&mut self, i: u64) {
        if i == 0 {
            self.data.push(0);
            return;
        }

        if i >= 2_u64.pow(35) { panic!("Integer value too large for VarInt") }

        let mut x = i;
        while x != 0 {
            let value = (x & 0b1111111) as u8;  // Get lowest 7 bits
            x >>= 7;  // Shift out read bits
            let bin_val = (((x != 0) as u8) << 7) + value;  // Set MSb if more data to come
            self.data.push(bin_val);
        }
    }

    pub fn write_u16(&mut self, i: u16) {
        self.data.extend_from_slice(&i.to_be_bytes());
    }

    pub fn write_i64(&mut self, i: i64) {
        self.data.extend_from_slice(&i.to_be_bytes());
    }

    pub fn write_string(&mut self, s: &str) {
        // Write size (as VarInt), then UTF-8 string
        self.write_var_int(s.len() as u64);
        self.data.extend_from_slice(s.as_bytes());
    }

    // Readers
    pub fn read_var_int(s: &mut dyn Read) -> Result<u64, PacketParseError> {
        let mut val: u64 = 0;
        let mut i = 0;
        let mut iter = s.bytes();

        loop {
            let byte = iter.next().ok_or_else(|| PacketParseError::new("Ran out of bytes trying to read VarInt"))?
                .map_err(|err| PacketParseError(format!("OS Error reading VarInt: {}", err)))?;
            val |= ((byte & 0b01111111) as u64) << i;
            i += 7;

            if (byte & 0b10000000) == 0 { break }  // Reached end of VarInt
            if i > 35 { panic!("VarInt#from_stream: VarInt too large") }
        }
        Ok(val)
    }

    pub fn read_string(s: &mut dyn Read) -> Result<String, PacketParseError> {
        let size = MCPacket::read_var_int(s)?;
        let mut buf = vec![0; size as usize];
        s.read_exact(buf.as_mut_slice()).map_err(|_| PacketParseError::new("Error reading string from stream"))?;
        String::from_utf8(buf).map_err(|err| PacketParseError(format!("Error decoding string: {}", err)))
    }

    //
    // Static helpers
    //
    pub fn status_handshake(address: &str, port: u16) -> Self {
        // Hard-coding protocol version for now
        let mut packet = Self::new(0);
        packet.write_var_int(760);  // Protocol version
        packet.write_string(address);  // Server Address
        packet.write_u16(port);  // Server Port
        packet.write_var_int(1);  // Next State
        packet
    }

    //
    // Private
    //
    fn int_to_var_int(i: u64) -> Vec<u8>{
        if i == 0 {
            return vec!(0);
        }
        if i >= 2_u64.pow(35) { panic!("Integer value too large for VarInt") }

        let mut x = i;
        let mut buf = Vec::new();
        while x != 0 {
            let value = (x & 0b1111111) as u8;  // Get lowest 7 bits
            x >>= 7;  // Shift out read bits
            let bin_val = (((x != 0) as u8) << 7) + value;  // Set MSb if more data to come
            buf.push(bin_val);
        }
        buf
    }
}
