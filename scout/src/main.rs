mod threads;
mod packet_handler;
mod config;

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{result, thread};
use std::error::Error;
use clap::{Parser, ArgGroup};
use ipnet::Ipv4AddrRange;
use pnet::datalink::NetworkInterface;
use serde_json::Value;

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
    // TODO reorganize dependencies
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
            println!("Received Ctrl+C!");
            stop_signal.store(true, Ordering::Relaxed);
        }).expect("Error setting Ctrl+C handler");
    }

    let sender_finish_signal = Arc::new(AtomicBool::new(false));

    // Start receiver thread
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
        let handle;


        // Start sender thread
        // TODO With new architecture, we only have 1 thread and we block until
        //  it is done. Can refactor into function instead of thread?
        {
            let iface = interface.clone();
            let sender_finish_signal = sender_finish_signal.clone();
            handle = thread::spawn(move || {
                threads::sender_thread(&iface, &ips, &sender_finish_signal);
            });
        }

        // Block until sender is done
        handle.join().expect("sender thread panic!");

        println!("Finished job #{}", job_id);
        confirm_job(job_id);

        // Lock ip vec mutex
        let mut valid_ips = valid_ips_mtx.lock().unwrap();
        println!("{:?}", valid_ips);
        if upload_ips(&valid_ips) {
            // Successful upload
            println!("Successfully uploaded job result");
        } else {
            println!("Could not upload job to dispatch server");
            break;
        }
        // Done using ip vector, clear vector and set finish signal to false again
        valid_ips.clear();
        sender_finish_signal.store(false, Ordering::Relaxed);
    }

    receiver_handle.join().expect("receiver thread panic!");
    Ok(())
}

fn confirm_job(id: u32) -> bool{
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/scout/job/{}", config::get_dispatcher_base(), id);
    client.post(url).send().is_ok()
}

fn upload_ips(ips: &Vec<Ipv4Addr>) -> bool {
    // let ips_json = serde_json::to_string(ips).unwrap();

    let client = reqwest::blocking::Client::new();
    let url = format!("{}/scout/ips", config::get_dispatcher_base());
    client.post(url).json(ips).send().is_ok()
}

fn get_job() -> (Vec<Ipv4Addr>, u32) {
    let url = format!("{}/scout/job/{}", config::get_dispatcher_base(), config::get_job_size());
    let res = reqwest::blocking::get(url)
        .unwrap().text().unwrap();
    let json: Value = serde_json::from_str(&res).unwrap();
    let job_id: u32 = json["id"].as_u64().unwrap() as u32;
    let job: Vec<Ipv4Addr> = Ipv4AddrRange::new(json["min"].as_str().unwrap().parse().unwrap(),
                                                json["max"].as_str().unwrap().parse().unwrap()).collect();
    (job, job_id)
}

fn print_adapter_info(adapter: &NetworkInterface) {
    let ip: String = match adapter.ips.get(0) {
        Some(ip) => ip.ip().to_string(),
        None => String::from("No IP")
    };
    println!("Interface: {} - {} - {}", adapter.name, ip, adapter.mac.unwrap())
}
