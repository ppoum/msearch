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
            println!("Received SIGINT signal, stopping client");
            stop_signal.store(true, Ordering::Relaxed);
        }).expect("Error setting SIGINT handler");
    }

    while !stop_signal.load(Ordering::Relaxed) {
        println!("Trying to obtain an IP to scan");
        let ip = get_ip();
        if ip.is_none() {
            println!("No IP available yet, waiting...");
            sleep(Duration::from_secs(5));
            continue;
        }

        let ip = ip.unwrap();
        println!("Scanning {}", ip);
        if let Ok(json) = validate_server(ip) {
            println!("SERVER FOUND!\nDesc: {}\nPlayers: {}/{}\n{}", json["description"],
                     json["players"]["online"], json["players"]["max"], json["players"]["sample"]);
        }
        // TODO push results to dispatcher (MC-10) (push even if no server found, to confirm proper
        //  deep scan of server
    }

    Ok(())
}

fn get_ip() -> Option<Ipv4Addr> {
    let url = format!("{}/client/job", config::get_dispatcher_base());
    let res = reqwest::blocking::get(url).unwrap();

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return None;
    }
    let t = res.text().unwrap();
    Some(t.parse().unwrap())
}
