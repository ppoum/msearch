mod data_types;
mod checker;
mod ip_dispatcher;

extern crate pnet;

use std::io::Result;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use ip_dispatcher::IpDispatcher;
use pnet::packet::Packet;

pub const JOB_SIZE: usize = 256;
pub const TCP_TIMEOUT_SECS: u64 = 2;
const THREAD_COUNT: usize = 1;
const STATUS_FREQ: usize = 1;

fn main() -> Result<()> {
    println!("Launching");

    let mut stop_signal = Arc::new(AtomicBool::new(false));
    // stop_signal.store(true, Ordering::Relaxed);

    let job_dispatch = Arc::new(Mutex::new(IpDispatcher::new()));

    let mut thread_handles = Vec::new();
    for tid in 0..THREAD_COUNT {
        let stop_sig_copy = Arc::clone(&stop_signal);
        let job_dispatch = job_dispatch.clone();
        thread_handles.push(thread::spawn(move || {
            thread_main(tid, &stop_sig_copy, &job_dispatch);
        }));
    }

    for t in thread_handles {
        t.join().expect("thread panic!");
    }

    Ok(())
}

fn thread_main(id: usize, stop_signal: &AtomicBool, job_dispatch_mtx: &Mutex<IpDispatcher>) {
    println!("Launching thread #{}", id);

    while !stop_signal.load(Ordering::Relaxed) {
        let job;
        {
            job = job_dispatch_mtx.lock().unwrap().get_job();
        }
        for (i, addr) in job.into_iter().enumerate() {
            // Every n tries, print check if stop signal and print progress status
            if i % STATUS_FREQ == 0 {
                if stop_signal.load(Ordering::Relaxed) { break }
                println!("Trying {} on thread #{}", addr, id);
            }
            if let Ok(json) = checker::validate_server(addr) {
                // Important data in JSON object: version.name, players.{online, max}, players.sample[?].name, description.text, favicon
                println!("VALID: {} (thread {})\nDesc: {}\n Players: {}/{}\n{}",
                         addr, id, json["description"]["text"],
                         json["players"]["online"], json["players"]["max"], json["players"]["sample"]);
            }
        }
    }
}
