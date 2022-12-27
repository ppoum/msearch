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
    let (mut tx, mut rx) = match pnet::datalink::channel(iface, pnet_config) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Wrong chanel type"),
        Err(e) => panic!("Error creating channel: {}", e)
    };

    // // Get packets until signal to stop received. Check for signal after 5s w/o packet

    // Get packets, when stop signal set to true, receive until no packets during timeout time period
    let max_no_packet_period = config::get_receive_timeout();
    let mut last_packet_time = SystemTime::now();
    let mut awaiting_sanity = false;
    let mut sanity_cnt = 0;

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
                if awaiting_sanity && validate_sanity_reply(packet) {
                    // This was the reply to the sanity packet, don't analyze it further
                    awaiting_sanity = false;
                    continue;
                }

                if let Some((ip, syn_ack)) = validate_response(packet, 25565) {
                    // Response from server

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
                    // Can't stop yet, check if no packets received due to rate limiting
                    // Send sanity SYN to 1.1.1.1 (Cloudflare) and try to get an answer
                    // If no answer, fair bet to assume rate limit, sleep for longer

                    if awaiting_sanity && sanity_cnt < 5 {  // TODO Move 5 to config
                        // Already sent out sanity packet, no reply, wait for longer
                        sanity_cnt += 1;  // One more timeout awaiting sanity reply
                        println!("No sanity reply, sleeping for {} secs...", 2);
                        sleep(Duration::from_secs(2));  // TODO Move to config file
                        continue;
                    }

                    // No answer or sent sanity packet too long ago, (re)send sanity packet
                    println!("No answer in a while, sending sanity packet");
                    tx.build_and_send(1, 66, &mut |packet: &mut [u8]| {
                        generate_syn_packet(iface, &Ipv4Addr::new(1, 1, 1, 1), 80, packet);
                    });
                    sanity_cnt = 0;
                    awaiting_sanity = true;
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
