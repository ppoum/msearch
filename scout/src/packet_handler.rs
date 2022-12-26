use std::net::{IpAddr, Ipv4Addr};
use pnet::datalink::{MacAddr, NetworkInterface};
use pnet::packet::ethernet::{EthernetPacket, EtherTypes, MutableEthernetPacket};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::{Ipv4Flags, Ipv4Packet, MutableIpv4Packet};
use pnet::packet::Packet;
use pnet::packet::tcp::{MutableTcpPacket, TcpFlags, TcpOption, TcpPacket};

pub fn generate_syn_packet(iface: &NetworkInterface, dest_ip: &Ipv4Addr, port: u16, pkt: &mut [u8]) {
    const L2_SIZE: usize = 14;
    const L3_SIZE: usize = 20;

    let src_ip = match iface.ips.get(0).unwrap().ip() {
        IpAddr::V4(addr) => addr,
        _ => panic!("Interface is ipv6")
    };

    // Generate L2 header
    let mut eth_pkt = MutableEthernetPacket::new(&mut pkt[..L2_SIZE]).unwrap();
    eth_pkt.set_destination(MacAddr::broadcast());
    eth_pkt.set_source(iface.mac.unwrap());
    eth_pkt.set_ethertype(EtherTypes::Ipv4);

    // Generate L3 header
    let mut ip_pkt = MutableIpv4Packet::new(&mut pkt[L2_SIZE..(L2_SIZE + L3_SIZE)]).unwrap();
    ip_pkt.set_header_length(69);
    ip_pkt.set_total_length(52);
    ip_pkt.set_next_level_protocol(IpNextHeaderProtocols::Tcp);
    ip_pkt.set_source(src_ip);
    ip_pkt.set_destination(*dest_ip);
    ip_pkt.set_identification(rand::random::<u16>());
    // ip_pkt.set_identification(IP_ID);
    ip_pkt.set_ttl(64);
    ip_pkt.set_version(4);
    ip_pkt.set_flags(Ipv4Flags::DontFragment);
    let checksum = pnet::packet::ipv4::checksum(&ip_pkt.to_immutable());
    ip_pkt.set_checksum(checksum);

    // Generate L4 header
    let mut tcp_pkt = MutableTcpPacket::new(&mut pkt[(L2_SIZE + L3_SIZE)..]).unwrap();
    tcp_pkt.set_source(6900);  // Set to 6900+threadid later
    tcp_pkt.set_destination(port);
    tcp_pkt.set_flags(TcpFlags::SYN);
    tcp_pkt.set_window(64240);
    tcp_pkt.set_data_offset(8);
    tcp_pkt.set_urgent_ptr(0);
    tcp_pkt.set_sequence(0);
    tcp_pkt.set_options(&[TcpOption::mss(1460), TcpOption::sack_perm(), TcpOption::nop(), TcpOption::nop(), TcpOption::wscale(7)]);
    let checksum = pnet::packet::tcp::ipv4_checksum(&tcp_pkt.to_immutable(), &src_ip, dest_ip);
    tcp_pkt.set_checksum(checksum);
}

///
///
/// # Arguments
///
/// * `packet`: The bytearray representation of the packet
/// * `port`: The expected source port
///
/// returns: Option<(Ipv4Addr, bool)>
/// * `Ipv4Addr`: The source address of the packet
/// * `bool`: True if the packet was a TCP packet with SYN and ACK flags
///
pub fn validate_response(packet: &[u8], port: u16) -> Option<(Ipv4Addr, bool)> {
    let ethernet = match EthernetPacket::new(packet) {
        Some(x) => x,
        None => return None
    };

    let ipv4 = match Ipv4Packet::new(ethernet.payload()) {
        Some(x) => x,
        None => return None
    };

    let tcp = match TcpPacket::new(ipv4.payload()) {
        Some(x) => x,
        None => return None
    };
    if tcp.get_source() != port { return None; }  // Wrong port

    // println!("{}:{} --> {}:{}",
    //         ipv4.get_source(), tcp.get_source(),
    //         ipv4.get_destination(), tcp.get_destination());

    let is_syn_ack = (tcp.get_flags() & TcpFlags::SYN) != 0 && (tcp.get_flags() & TcpFlags::ACK) != 0;
    Some((ipv4.get_source(), is_syn_ack))
}