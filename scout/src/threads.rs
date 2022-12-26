use std::io::{stdout, Write};
use std::net::Ipv4Addr;
use std::ops::Add;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use pnet::datalink::{Channel, Config, NetworkInterface};
use crate::config;
use crate::packet_handler::*;

/// The sender thread sends out a single SYN packet to every IP address specified in the list.
/// Once finished, it signals to the main thread using sender_finish_signal that it is done with the
/// current IP batch.
///
/// # Arguments
///
/// * `iface`: The interface to use when sending out the packets
/// * `ips`: The batch of IPs
/// * `sender_finish_signal`: Gets set to true once batch is done
///
pub fn sender_thread(iface: &NetworkInterface, ips: &Vec<Ipv4Addr>, sender_finish_signal: &AtomicBool) {
    let (mut tx, _) = match pnet::datalink::channel(iface, Default::default()) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Wrong chanel type"),
        Err(e) => panic!("Error creating channel: {}", e)
    };
    println!("Sending new packets");

    let time_per_packet = Duration::from_micros((1000000.0 / config::get_send_rate() as f64) as u64);
    println!("TPP: {} ms", time_per_packet.as_millis());
    for ip in ips {
        tx.build_and_send(1, 66, &mut |packet: &mut [u8]| {
            generate_syn_packet(iface, ip, 25565, packet);
        });
        sleep(time_per_packet);
    }
    sender_finish_signal.store(true, Ordering::Relaxed);
}

/// The receiver thread receives the answer (SYN ACK) to our packets. It tries to receive
/// until the stop_signal is set to true, at which point it waits until a specified amount of time
/// passes without having received an answer.
///
/// When `sender_finish_signal` is true, release the ip vector mutex to allow the main thread to
/// read it.
///
/// The thread only considers a packet as a valid response if it is a SYN ACK packet originating
/// from port 25565. This value is currently hard-coded and could be variable in the future.
///
/// # Arguments
///
/// * `iface`: The interface to receive packets on.
/// * `valid_ips_mtx`: IP vectors containing all IPs that sent a valid answer
/// * `stop_signal`: When true, start stop process described above.
/// * `sender_finish_signal`: Should be set to true when the sender is done with its batch
///
pub fn receiver_thread(iface: &NetworkInterface, valid_ips_mtx: &Mutex<Vec<Ipv4Addr>>, stop_signal: &AtomicBool, sender_finish_signal: &AtomicBool) {
    // Create channel (get packets with a timeout of 1s)
    let pnet_config = Config {
        read_timeout: Option::from(Duration::from_secs(1)),
        ..Default::default()
    };
    let (_, mut rx) = match pnet::datalink::channel(iface, pnet_config) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Wrong chanel type"),
        Err(e) => panic!("Error creating channel: {}", e)
    };

    // // Get packets until signal to stop received. Check for signal after 5s w/o packet
    // let mut packet_count: u32 = 0;
    // let mut time_waited: u64 = 0;

    // Get packets, when stop signal set to true, receive until no packets during timeout time period
    let max_no_packet_period = config::get_receive_timeout();
    let mut last_packet_time = SystemTime::now();

    let mut valid_ips = valid_ips_mtx.lock().unwrap();

    // Counters
    let mut valid_count = 0;
    let mut invalid_count = 0;
    loop {
        // This logic of dropping and re-obtaining mutex is somewhat ugly
        // is there a better way?
        if sender_finish_signal.load(Ordering::Relaxed) {
            // Sender is done, release mutex
            drop(valid_ips);

            // Wait until sender has new task
            while sender_finish_signal.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(10));
            }
            valid_ips = valid_ips_mtx.lock().unwrap();
        }

        match rx.next() {
            Ok(packet) => {
                if let Some((ip, syn_ack)) = validate_response(packet, 25565) {
                    // Response from server

                    // packet_count += 1;
                    // print!("\rResponses: {}", packet_count);
                    // println!("Response from: {}", ip);
                    last_packet_time = SystemTime::now();
                    if syn_ack {
                        // Valid response packet
                        valid_ips.push(ip);
                        valid_count += 1;
                    } else {
                        // Invalid response (i.e. SYN RST)
                        invalid_count += 1;
                    }
                    print!("\rBad responses: {}, valid responses: {}", invalid_count, valid_count);
                    stdout().flush().expect("Error flushing stdout");
                }
            }
            _ => {
                // No packets received
                if !stop_signal.load(Ordering::Relaxed) {
                    // Can't stop yet
                    continue;
                }

                // Received stop signal
                let time_since_packet = SystemTime::now().duration_since(last_packet_time)
                    .expect("Error comparing time, clock may have changed.");
                if time_since_packet > max_no_packet_period {
                    break;
                }

                // Can't stop yet
                let time_stop = last_packet_time.add(max_no_packet_period);  // Time when we can break
                let wait_period = time_stop.duration_since(SystemTime::now()).expect("Error time");
                print!("\rStopping in {:.2} secs", wait_period.as_secs_f64());
                stdout().flush().expect("Error flushing stdout");
            }
        }
    }
    println!("\nStopping receiver thread");
}
