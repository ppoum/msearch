use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;
use itertools::Itertools;
use rocket::{get, Route, State};
use rocket::response::status;
use rocket::response::status::BadRequest;
use crate::ServerState;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, json, Serialize, Serializer};
use rocket::serde::ser::SerializeStruct;

static JOB_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ScoutJob {
    id: u32,
    min: Ipv4Addr,
    max: Ipv4Addr
}

// Implement default since scout does not send creation_time in its request
impl Default for ScoutJob {
    fn default() -> Self {
        ScoutJob {
            id: 0,
            min: Ipv4Addr::new(0, 0, 0, 0),
            max: Ipv4Addr::new(0, 0, 0, 0)
        }
    }
}

impl Serialize for ScoutJob {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut state = serializer.serialize_struct("IpRange", 2)?;
        state.serialize_field("min", &self.min.to_string())?;
        state.serialize_field("max", &self.max.to_string())?;
        state.serialize_field("id", &self.id)?;
        state.end()
    }
}

pub fn get_all_routes() -> Vec<Route> {
    routes![get_job, post_ips]
}


// ROUTES

#[get("/job/<size>")]
fn get_job(size: usize, state: &State<ServerState>) -> Json<ScoutJob> {
    let mut range = state.ip_range.lock().unwrap();
    let x: Vec<Ipv4Addr> = range.take(size).collect();

    // Manually advance original iterator
    for _ in 0..size {
        if range.next().is_none() {
            // Reached end of iterator, reset it and exit loop
            state.reset_ip_range();
            break;
        }
    }

    let new_job = ScoutJob {
        min: *x.get(0).unwrap(),
        max: *x.last().unwrap(),
        id: JOB_ID.fetch_add(1, Ordering::SeqCst)
    };
    Json(new_job)
}

#[post("/ips", data = "<json>")]
fn post_ips(json: &str, state: &State<ServerState>) -> Result<status::Accepted<String>, BadRequest<String>> {
    let ips: Vec<String> = json::from_str(json).map_err(|e| BadRequest(Some(e.to_string())))?;
    let ips: VecDeque<Ipv4Addr> = ips.iter().map(|x| x.parse().unwrap()).unique().collect();
    println!("Received the following ips: {:?}", ips);

    if ips.is_empty() {
        // Return before trying to gain mutex lock
        return Ok(status::Accepted(None));
    }

    let mut valid_ips = state.valid_ips.lock().unwrap();
    // Costly iteration, could get out of hand if ip backlog is too large?
    // Consider using faster lookup data type, like hash list
    for ip in ips {
        if !valid_ips.contains(&ip) {
            valid_ips.push_back(ip);
        }
    }

    Ok(status::Accepted(None))
}
