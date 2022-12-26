#[macro_use] extern crate rocket;

mod routes;

use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::sync::Mutex;
use ipnet::Ipv4AddrRange;
use routes::scout_routes;
use scout_routes::ScoutJob;
use crate::routes::{client_routes, info_routes};

pub struct ServerState {
    ip_range: Mutex<Ipv4AddrRange>,
    valid_ips: Mutex<VecDeque<Ipv4Addr>>
}

impl ServerState {
    pub fn reset_ip_range(&self) {
        let mut range = self.ip_range.lock().expect("Error locking mutex");
        *range = Ipv4AddrRange::new("1.0.0.0".parse().unwrap(),
                                    "255.255.255.255".parse().unwrap());
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/scout", scout_routes::get_all_routes())
        .mount("/info", info_routes::get_all_routes())
        .mount("/client", client_routes::get_all_routes())
        .manage(ServerState {
            ip_range: Mutex::new(Ipv4AddrRange::new("1.0.0.0".parse().unwrap(),
            "255.255.255.255".parse().unwrap())),
            valid_ips: Mutex::new(VecDeque::new())
        })
}