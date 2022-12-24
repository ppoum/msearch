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
    outstanding_scout_jobs: Mutex<Vec<ScoutJob>>,  // TODO Make queue?
    valid_ips: Mutex<VecDeque<Ipv4Addr>>  // TODO Make queue instead of vector
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
    // TODO Cleanup thread that adds unresolved ip chunks to priority queue
    // TODO Create InfiniteIpv4AddrRange struct, which auto loops when all ips have
    //  have been exhausted and skips LAN ranges
    // TODO Add queue for outstanding jobs if scout client does not report back completion status after n time
    //  Add job chunk back to pool if no answer from client (separate priority pool to avoid messing with IP range pool?)
    // TODO Save state on shutdown (store in file ips, ip range, etc...)
    rocket::build()
        .mount("/scout", scout_routes::get_all_routes())
        .mount("/info", info_routes::get_all_routes())
        .mount("/client", client_routes::get_all_routes())
        .manage(ServerState {
            ip_range: Mutex::new(Ipv4AddrRange::new("1.0.0.0".parse().unwrap(),
            "255.255.255.255".parse().unwrap())),
            outstanding_scout_jobs: Mutex::new(Vec::new()),
            valid_ips: Mutex::new(VecDeque::new())
        })
}