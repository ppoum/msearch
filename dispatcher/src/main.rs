#[macro_use] extern crate rocket;

mod routes;
mod ip_chunk_iterator;

use std::collections::VecDeque;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Mutex;
use rocket::serde::{Deserialize, Serialize};
use routes::scout_routes;
use crate::ip_chunk_iterator::IpChunkIterator;
use crate::routes::{client_routes, info_routes};
use crate::routes::client_routes::ClientJob;

#[derive(Serialize, Deserialize)]
pub struct ServerState {
    ip_range: Mutex<IpChunkIterator>,
    valid_ips: Mutex<VecDeque<Ipv4Addr>>,
    outstanding_client_jobs: Mutex<VecDeque<ClientJob>>
}

#[rocket::main]
async fn main() {
    println!("Trying to load previously saved server state");
    let server_state: ServerState;
    if Path::new("./.dispatcher").exists() {
        println!("Found file, loading it");
        let data = fs::read_to_string("./.dispatcher").expect("Unable to read file");
        server_state = rocket::serde::json::from_str(&data).expect("Invalid file contents");
    } else {
        println!("No saved state file found.");
        server_state = ServerState {
            ip_range: Mutex::new(IpChunkIterator::new()),
            valid_ips: Mutex::new(VecDeque::new()),
            outstanding_client_jobs: Mutex::new(VecDeque::new())
        };
    }

    let result = rocket::build()
        .mount("/scout", scout_routes::get_all_routes())
        .mount("/info", info_routes::get_all_routes())
        .mount("/client", client_routes::get_all_routes())
        .manage(server_state)
        .launch()
        .await.expect("Server failed unexpectedly");

    println!("Saving current state to disk.");
    let server_state = result.state::<ServerState>().unwrap();
    // Remove outstanding jobs, add back to queue
    let mut outstanding = server_state.outstanding_client_jobs.lock().unwrap();
    let mut valid_ips = server_state.valid_ips.lock().unwrap();
    outstanding.iter().for_each(|x| valid_ips.push_front(x.ip));
    outstanding.clear();
    // Serialize to json, save to file
    let json_state = rocket::serde::json::to_string(server_state)
        .expect("Error serializing server state");
    fs::write("./.dispatcher", json_state).expect("Unable to save state to disk");
    println!("Success! Shutting down...")
}
