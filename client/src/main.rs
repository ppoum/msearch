mod mc_packet;
mod checker;
mod config;

extern crate pnet;

use std::io::Result;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;
use clap::Parser;
use serde_json::{json, Value};
use crate::checker::validate_server;

pub const JOB_SIZE: usize = 256;
pub const TCP_TIMEOUT_SECS: u64 = 2;

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
struct Cli {
    #[arg(short, long="config")]
    config_path: String
}


fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Err(err) = config::load_config(&cli.config_path) {
        panic!("Received the following error when parsing config file:\n{}", err);
    }


    // Setup SIGINT handler
    let stop_signal = Arc::new(AtomicBool::new(false));
    {
        let stop_signal = stop_signal.clone();
        ctrlc::set_handler(move || {
            if stop_signal.load(Ordering::Relaxed) {
                println!("Received second SIGINT, forcefully exiting.");
                std::process::exit(1);
            } else {
                println!("Received SIGINT signal, stopping client");
                stop_signal.store(true, Ordering::Relaxed);
            }
        }).expect("Error setting SIGINT handler");
    }

    while !stop_signal.load(Ordering::Relaxed) {
        println!("Trying to obtain an IP to scan");
        let job = get_job();
        if job.is_none() {
            println!("No IP available yet, waiting...");
            sleep(Duration::from_secs(5));
            continue;
        }

        let (id, ip) = job.unwrap();
        println!("Scanning {}", ip);
        if let Ok(json) = validate_server(ip) {
            send_results(id, ip, Some(&json));
            let json: Value = serde_json::from_str(&json)?;
            println!("SERVER FOUND!\nDesc: {}\nPlayers: {}/{}\n{}", json["description"],
                     json["players"]["online"], json["players"]["max"], json["players"]["sample"]);
        } else {
            send_results(id, ip, None);
        }
    }

    Ok(())
}

fn get_job() -> Option<(u32, Ipv4Addr)> {
    let url = format!("{}/client/job", config::get_dispatcher_base());
    let client = reqwest::blocking::Client::new();
    let res;
    loop {
        match client.get(&url).send() {
            Ok(r) => {
                res = r;
                break;
            }
            Err(_) => {
                println!("Error sending request to server, retrying in 5 seconds.");
                sleep(Duration::from_secs(5));
            }
        }
    }

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return None;
    } else if res.status() != reqwest::StatusCode::OK {
        println!("Unknown status code received from dispatch server.");
        return None;
    }

    let t = res.text().unwrap();
    let v: Value = match serde_json::from_str(&t) {
        Ok(v) => v,
        Err(_) => return None
    };

    // Validate id field
    let id = match v["id"].as_str() {
        Some(s) => s,
        None => return None
    };
    let id = match id.parse() {
        Ok(u) => u,
        Err(_) => return None
    };

    // Validate IP field
    let ip = match v["ip"].as_str() {
        Some(s) => s,
        None => return None
    };
    let ip = match ip.parse() {
        Ok(i) => i,
        Err(_) => return None
    };

    Some((id, ip))
}

fn send_results(id: u32, ip: Ipv4Addr, response: Option<&str>) {
    let url = format!("{}/client/job/{}", config::get_dispatcher_base(), id);
    let body = match response {
        Some(response) => json!({
            "status": "up",
            "ip": ip.to_string(),
            "response": response
        }),
        None => json!({
            "status": "down",
            "ip": ip.to_string(),
        })
    };

    let client = reqwest::blocking::Client::new();
    loop {
        match client.post(&url).json(&body).send() {
            Ok(_) => break,
            Err(_) => {
                println!("Error uploading data to server, retrying in 5 seconds.");
                sleep(Duration::from_secs(5));
            }
        }
    }
}
