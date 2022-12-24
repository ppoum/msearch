use std::io::{Result, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;
use serde_json::Value;
use crate::data_types::var_int::*;
use crate::data_types::protocol_string::*;

// TODO rewrite as MCPacket class, with writeVarInt, writeString, etc... methods

pub fn validate_server(addr: Ipv4Addr) -> Result<Value> {
    let socket_addr = SocketAddr::new(IpAddr::V4(addr), 25565);
    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(crate::TCP_TIMEOUT_SECS))?;

    // Initialize MC connection
    send_handshake(&mut stream, &socket_addr.ip().to_string(), socket_addr.port())?;

    // Ask for ping info
    send_status_request(&mut stream)?;

    // Get server response
    receive_answer(&mut stream)
}

fn send_handshake(s: &mut TcpStream, address: &str, port: u16) -> Result<()> {
    let mut buf: Vec<u8> = Vec::new();

    // Write packet ID
    VarInt::from_int(0).write_to_stream(&mut buf)?;

    // Write protocol version
    VarInt::from_int(1).write_to_stream(&mut buf)?;

    // Write address size, followed by address string
    ProtocolString::from_str(address).write_to_stream(&mut buf)?;

    // Write port
    buf.write_all(&port.to_be_bytes())?;

    // Write next state (status)
    VarInt::from_int(1).write_to_stream(&mut buf)?;

    // Get buffer length, write to TCP stream as VarInt
    VarInt::from_int(buf.len() as u64).write_to_stream(s)?;

    // Write buffer to TCP stream
    s.write_all(&buf)?;

    // Send
    s.flush()?;

    Ok(())
}

fn send_status_request(s: &mut TcpStream) -> Result<()> {
    // No data, only packet ID - length is always 1
    // Full packet will be VarInt(1) and VarInt(0)
    VarInt::from_int(1).write_to_stream(s)?;
    VarInt::from_int(0).write_to_stream(s)?;

    // Send
    s.flush()?;

    Ok(())
}

fn receive_answer(s: &mut TcpStream) -> Result<Value> {
    let _packet_size = VarInt::from_stream(s);  // Forward read pointer
    let _packet_id = VarInt::from_stream(s);  // Forward read pointer
    let json_str = ProtocolString::from_stream(s);
    let json: Value = serde_json::from_str(json_str.to_str())?;
    // println!("{}", json.to_str());
    println!("SERVER FOUND!\nDesc: {}\nPlayers: {}/{}\n{}", json["description"],
             json["players"]["online"], json["players"]["max"], json["players"]["sample"]);

    Ok(json)
}