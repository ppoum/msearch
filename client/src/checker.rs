use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;
use serde_json::Value;
use crate::mc_packet::{MCPacket, PacketParseError};

#[derive(Debug)]
pub struct InvalidServerError(String);

impl Display for InvalidServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error occurred when trying to validate server: {}", self.0)
    }
}

impl Error for InvalidServerError {}

pub fn validate_server(addr: Ipv4Addr) -> Result<Value, InvalidServerError> {
    let socket_addr = SocketAddr::new(IpAddr::V4(addr), 25565);
    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(crate::TCP_TIMEOUT_SECS))
        .map_err(|_| InvalidServerError("Timed out connecting to host".into()))?;
    stream.set_read_timeout(Some(Duration::from_secs(crate::TCP_TIMEOUT_SECS)))
        .map_err(|_| InvalidServerError("Error when trying to set read timeout".into()))?;

    // Initialize MC connection
    MCPacket::status_handshake(&addr.to_string(), 25565).write_to_stream(&mut stream);

    // Ask for ping info
    MCPacket::new(0).write_to_stream(&mut stream);

    // Get server response
    receive_status_response(&mut stream).map_err(|err| InvalidServerError(err.message()))
}

fn receive_status_response(s: &mut TcpStream) -> Result<Value, PacketParseError> {
    // Skip header
    let _packet_size = MCPacket::read_var_int(s)?;
    let _packet_id = MCPacket::read_var_int(s)?;

    let json_str = MCPacket::read_string(s)?;
    let json: Value = serde_json::from_str(&json_str).map_err(|_| PacketParseError("Error decoding the status JSON message".into()))?;
    Ok(json)
}
