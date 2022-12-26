use std::net::Ipv4Addr;
use ipnet::Ipv4AddrRange;

pub struct IpDispatcher {
    ip_range: Ipv4AddrRange
}

impl IpDispatcher {
    pub fn new() -> IpDispatcher {
        let ip_range = Ipv4AddrRange::new("1.0.6.0".parse().unwrap(),
                                          "255.255.255.255".parse().unwrap());
        IpDispatcher{ ip_range }
    }

    pub fn get_job(&mut self) -> Vec<Ipv4Addr> {
        let x = self.ip_range.take(crate::JOB_SIZE).collect();

        // advance_by and next_chunk are both only in nightly, manually advance
        for _ in 0..crate::JOB_SIZE {
            self.ip_range.next();
        }

        x
    }
}