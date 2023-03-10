use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;
use crate::mc_packet::{MCPacket, PacketParseError};

#[derive(Debug)]
pub struct InvalidServerError(String);

impl InvalidServerError {
    pub fn new(s: &str) -> Self {
        Self(String::from(s))
    }
}

impl Display for InvalidServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error occurred when trying to validate server: {}", self.0)
    }
}

impl Error for InvalidServerError {}

pub fn validate_server(addr: Ipv4Addr) -> Result<String, InvalidServerError> {
    let socket_addr = SocketAddr::new(IpAddr::V4(addr), 25565);
    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(crate::TCP_TIMEOUT_SECS))
        .map_err(|_| InvalidServerError::new("Timed out connecting to host"))?;
    stream.set_read_timeout(Some(Duration::from_secs(crate::TCP_TIMEOUT_SECS)))
        .map_err(|_| InvalidServerError::new("Error when trying to set read timeout"))?;

    // Initialize MC connection
    MCPacket::status_handshake(&addr.to_string(), 25565).write_to_stream(&mut stream);

    // Ask for ping info
    MCPacket::new(0).write_to_stream(&mut stream);

    // Get server response
    receive_status_response(&mut stream).map_err(|err| InvalidServerError(err.message()))
}

fn receive_status_response(s: &mut TcpStream) -> Result<String, PacketParseError> {
    // Skip header
    let _packet_size = MCPacket::read_var_int(s)?;
    let _packet_id = MCPacket::read_var_int(s)?;

    let json_str = MCPacket::read_string(s)?;
    Ok(json_str)
}
