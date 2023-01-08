extern crate env_logger;

mod routes;
mod ip_chunk_iterator;

use std::collections::VecDeque;
use std::{fs, io};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use actix_web::{App, HttpServer};
use actix_web::middleware::Logger;
use actix_web::web::Data;
use serde::{Deserialize, Serialize};
use tokio::{task, time};
use crate::ip_chunk_iterator::IpChunkIterator;
use crate::routes::client_routes::{ClientJob, get_client_scope};
use crate::routes::info_routes::get_info_scope;
use crate::routes::scout_routes::get_scout_scope;

#[derive(Serialize, Deserialize)]
pub struct ServerState {
    ip_range: Mutex<IpChunkIterator>,
    valid_ips: Mutex<VecDeque<Ipv4Addr>>,
    outstanding_client_jobs: Mutex<VecDeque<ClientJob>>
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    // Generate server state from saved state, or generate new
    println!("Trying to load previously saved server state");
    let server_state: Data<ServerState> = if Path::new("./.dispatcher").exists() {
        println!("Found file, loading it");
        let data = fs::read_to_string("./.dispatcher").expect("Unable to read file");
        Data::new(serde_json::from_str(&data).expect("Invalid file contents"))
    } else {
        println!("No saved state file found.");
        Data::new(ServerState {
            ip_range: Mutex::new(IpChunkIterator::new()),
            valid_ips: Mutex::new(VecDeque::new()),
            outstanding_client_jobs: Mutex::new(VecDeque::new())
        })
    };

    // Start recurring job
    {
        let server_state = server_state.clone();
        task::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;
                println!("Cleaning up outstanding client jobs.");
                let mut outstanding = server_state.outstanding_client_jobs.lock().unwrap();
                let mut valid_ips = server_state.valid_ips.lock().unwrap();

                outstanding.retain(|job| {
                    match SystemTime::now().duration_since(job.creation_time) {
                        Ok(d) => {
                            if d > Duration::from_secs(60) {
                                // Task is older than 1 minute, remove from outstanding, add back to valid_ips
                                println!("Removing job: {} - {}", job.id, job.ip);
                                let ip = job.ip;
                                valid_ips.push_front(ip);
                                return false;  // Remove from list
                            }
                            true  // Not too old yet, keep in list
                        },

                        Err(_) => true  // Error means creation_time more recent than system time, ignore error and keep job
                    }
                });
                println!("Finished cleaning outstanding client jobs.")
            }
        });
    }

    // Start web server
    env_logger::init();
    let state_copy = server_state.clone();
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(state_copy.clone())
            .service(get_client_scope())
            .service(get_scout_scope())
            .service(get_info_scope())
    }).bind(("0.0.0.0", 8000))?.run().await.expect("HttpServer panicked!");

    // Save to server state to disk
    println!("Saving current state to disk.");
    // Start by adding outstanding jobs back to valid_ips pool
    {
        let mut outstanding = server_state.outstanding_client_jobs.lock().unwrap();
        let mut valid_ips = server_state.valid_ips.lock().unwrap();
        outstanding.iter().for_each(|x| valid_ips.push_front(x.ip));
        outstanding.clear();
    }
    // Serialize to json, save to disk
    let json_val = serde_json::to_string(&server_state.into_inner())
        .expect("Error serializing server state");
    fs::write("./.dispatcher", json_val).expect("Unable to save state to disk.");
    println!("Success! Shutting down...");
    Ok(())
}
