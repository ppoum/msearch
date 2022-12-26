#[macro_use] extern crate rocket;

mod routes;
mod ip_chunk_iterator;

use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::sync::Mutex;
use rocket::serde::{Deserialize, Serialize};
use routes::scout_routes;
use crate::ip_chunk_iterator::IpChunkIterator;
use crate::routes::{client_routes, info_routes};

#[derive(Serialize, Deserialize)]
pub struct ServerState {
    ip_range: Mutex<IpChunkIterator>,
    valid_ips: Mutex<VecDeque<Ipv4Addr>>
}

#[rocket::main]
async fn main() {
    let result = rocket::build()
        .mount("/scout", scout_routes::get_all_routes())
        .mount("/info", info_routes::get_all_routes())
        .mount("/client", client_routes::get_all_routes())
        .manage(ServerState {
            ip_range: Mutex::new(IpChunkIterator::new()),
            valid_ips: Mutex::new(VecDeque::new())
        })
        .launch()
        .await.expect("Server failed unexpectedly");

    println!("Saving current state to disk.");
    let server_state = result.state::<ServerState>().unwrap();
    let json_state = rocket::serde::json::to_string(server_state)
        .expect("Error serializing server state");
    println!("{}", json_state)
}
