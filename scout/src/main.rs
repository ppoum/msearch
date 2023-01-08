mod threads;
mod packet_handler;
mod config;

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{result, thread};
use std::error::Error;
use std::thread::sleep;
use std::time::Duration;
use clap::{Parser, ArgGroup};
use pnet::datalink::{Channel, NetworkInterface};
use serde_json::Value;
use crate::packet_handler::generate_syn_packet;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(group(
ArgGroup::new("adapters")
.required(true)
.args(& ["list_adapters", "adapter"])
))]
struct Cli {
    #[arg(long)]
    list_adapters: bool,

    #[arg(long)]
    adapter: Option<String>,

    #[arg(short, long="config")]
    config_path: String,
}

pub type Result<T> = result::Result<T, Box<dyn Error>>;


fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_adapters {
        for iface in pnet::datalink::interfaces().iter() {
            print_adapter_info(iface);
        }
        return Ok(());
    }

    // Load config values from config file
    if let Err(e) = config::load_config(cli.config_path.as_str()) {
        panic!("Error trying to parse config file: {}", e);
    }

    println!("Launching");
    print!("Using adapter: ");

    let interface_name = cli.adapter.unwrap();
    let interfaces = pnet::datalink::interfaces();
    let interface = interfaces.iter()
        .find(|&iface| iface.name == interface_name).unwrap();
    print_adapter_info(interface);

    // Stop signal gets set to true when Ctrl+C received
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
        }).expect("Error setting Ctrl+C handler");
    }


    // Start receiver thread
    let sender_finish_signal = Arc::new(AtomicBool::new(false));
    let valid_ips_mtx = Arc::new(Mutex::new(Vec::new()));
    let receiver_handle;
    {
        let valid_ips = Arc::clone(&valid_ips_mtx);
        let stop_signal = stop_signal.clone();
        let sender_finish_signal = sender_finish_signal.clone();
        let iface = interface.clone();
        receiver_handle = thread::spawn(move || {
            threads::receiver_thread(&iface, &valid_ips, &stop_signal, &sender_finish_signal);
        });
    }

    // Send while we haven't received a stop signal
    while !stop_signal.load(Ordering::Relaxed) {
        let (ips, job_id) = get_job();

        // Send packets and signal receiver thread to release mutex to list of ips
        send_packets(interface, &ips);
        sender_finish_signal.store(true, Ordering::Relaxed);

        println!("Finished job #{}", job_id);

        // Lock ip vec mutex
        let mut valid_ips = valid_ips_mtx.lock().unwrap();
        println!("{:?}", valid_ips);
        while !upload_ips(&valid_ips) {
            println!("Error uploading job to dispatch server, retrying in 5 seconds.");
            sleep(Duration::from_secs(5));
        }
        println!("Successfully uploaded job result");
        valid_ips.clear();
        sender_finish_signal.store(false, Ordering::Relaxed);
    }

    receiver_handle.join().expect("receiver thread panic!");
    Ok(())
}

pub fn send_packets(iface: &NetworkInterface, ips: &Vec<Ipv4Addr>) {
    let (mut tx, _) = match pnet::datalink::channel(iface, Default::default()) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Wrong chanel type"),
        Err(e) => panic!("Error creating channel: {}", e)
    };
    println!("Sending new packets");

    let time_per_packet = Duration::from_micros((1000000.0 / config::get_send_rate() as f64) as u64);
    println!("TPP: {} us", time_per_packet.as_micros());
    for ip in ips {
        tx.build_and_send(1, 66, &mut |packet: &mut [u8]| {
            generate_syn_packet(iface, ip, 25565, packet);
        });
        sleep(time_per_packet);
    }
}

//
// Utility functions
//

fn upload_ips(ips: &Vec<Ipv4Addr>) -> bool {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/scout/ips", config::get_dispatcher_base());
    client.post(url).json(ips).send().is_ok()
}

fn get_job() -> (Vec<Ipv4Addr>, u32) {
    let url = format!("{}/scout/job/{}", config::get_dispatcher_base(), config::get_job_size());
    let client = reqwest::blocking::Client::new();
    let res;
    loop {
        match client.get(&url).send() {
            Ok(r) => {
                if r.status() == reqwest::StatusCode::OK {
                    res = r;
                    break;
                } else {
                    println!("Received invalid response from server, retrying in 5 seconds...");
                    sleep(Duration::from_secs(5));
                }
            },
            Err(_) => {
                println!("Error getting job from dispatch server, retrying in 5 seconds.");
                sleep(Duration::from_secs(5));
            }
        }
    }

    let res = res.text().unwrap();
    let json: Value = serde_json::from_str(&res).unwrap();
    let job_id: u32 = json["id"].as_u64().unwrap() as u32;
    let ips: Vec<Ipv4Addr> = json["ips"].as_array().unwrap().iter()
        .map(|x| x.as_str().unwrap().parse().unwrap()).collect();

    (ips, job_id)
}

fn print_adapter_info(adapter: &NetworkInterface) {
    let ip: String = match adapter.ips.get(0) {
        Some(ip) => ip.ip().to_string(),
        None => String::from("No IP")
    };
    println!("Interface: {} - {} - {}", adapter.name, ip, adapter.mac.unwrap())
}
