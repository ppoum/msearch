use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU32, Ordering};
use actix_web::{HttpResponse, Responder, Scope, Result, post, get, error};
use actix_web::web::{Data, Path, scope};
use itertools::Itertools;
use serde::{Serialize, Serializer};
use serde::ser::SerializeStruct;
use crate::ServerState;

static JOB_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct ScoutJob {
    id: u32,
    ips: Vec<Ipv4Addr>
}

impl Serialize for ScoutJob {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut state = serializer.serialize_struct("IpRange", 2)?;
        state.serialize_field("ips", &self.ips)?;
        state.serialize_field("id", &self.id)?;
        state.end()
    }
}

pub fn get_scout_scope() -> Scope {
    scope("/scout")
        .service(get_job)
        .service(post_ips)
}


// ROUTES

#[get("/job/{size}")]
async fn get_job(path: Path<usize>, state: Data<ServerState>) -> impl Responder {
    let size = path.into_inner();

    let mut ip_iterator = state.ip_range.lock().unwrap();
    let mut ips: Vec<Ipv4Addr> = Vec::new();

    while ips.len() < size {
        match ip_iterator.next() {
            Some(ip) => ips.push(ip),
            None => ip_iterator.regenerate()  // Iterator is empty, refill it
        }
    }

    let new_job = ScoutJob {
        ips,
        id: JOB_ID.fetch_add(1, Ordering::SeqCst)
    };

    HttpResponse::Ok().json(new_job)
}

#[post("/ips")]
async fn post_ips(json: String, state: Data<ServerState>) -> Result<impl Responder> {
    let ips: Vec<String> = serde_json::from_str(&json).map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    let ips: VecDeque<Ipv4Addr> = ips.iter().map(|x| x.parse().unwrap()).unique().collect();
    println!("Received the following ips: {:?}", ips);

    if ips.is_empty() {
        // Return before trying to gain mutex lock
        return Ok(HttpResponse::Ok().finish());
    }

    let mut valid_ips = state.valid_ips.lock().unwrap();
    // Costly iteration, could get out of hand if ip backlog is too large?
    // Consider using faster lookup data type, like hash list
    for ip in ips {
        if !valid_ips.contains(&ip) {
            valid_ips.push_back(ip);
        }
    }

    Ok(HttpResponse::Ok().finish())
}
