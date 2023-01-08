use std::net::{AddrParseError, Ipv4Addr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;
use actix_web::{HttpResponse, Scope, Result, Responder, error, get, post};
use actix_web::web::{Data, Path, scope};
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::SerializeStruct;
use serde_json::Value;
use crate::ServerState;

static JOB_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Deserialize, Copy, Clone)]
pub struct ClientJob {
    id: u32,
    pub ip: Ipv4Addr,
    creation_time: SystemTime
}

// Default used when deserializing w/ missing fields
impl Default for ClientJob {
    fn default() -> Self {
        ClientJob {
            id: 0,
            ip: Ipv4Addr::new(0, 0, 0, 0),
            creation_time: SystemTime::now()
        }
    }
}

impl Serialize for ClientJob {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut state = serializer.serialize_struct("ClientJob", 2)?;
        state.serialize_field("id", &self.id.to_string())?;
        state.serialize_field("ip", &self.ip.to_string())?;
        state.end()
    }
}

pub fn get_client_scope() -> Scope {
    scope("/client")
        .service(get_job)
        .service(post_job)
}

#[get("/job")]
async fn get_job(state: Data<ServerState>) -> Result<impl Responder> {
    let mut valid_ips = state.valid_ips.lock().unwrap();

    match valid_ips.pop_front() {
        Some(ip) => {
            let job = ClientJob {
                id: JOB_ID.fetch_add(1, Ordering::Relaxed),
                ip,
                creation_time: SystemTime::now()
            };
            // Add job to outstanding list
            let mut outstanding = state.outstanding_client_jobs.lock().unwrap();
            outstanding.push_back(job);

            Ok(HttpResponse::Ok().json(job))
        },
        None => Err(error::ErrorNotFound("No job available"))
    }
}

/// Body format:
/// ```json
/// {
///   "status": "up",
///   "ip": "0.0.0.0",
///   "response": {
///     ...
///   }
/// }
/// ```
/// `status` can take a value of `up`, or any other value meaning `down`<br>
/// `response` is an optional field and does not have to be provided if `status` is not `up`.
///  Must be included otherwise.
///
/// The response format is defined [here](https://wiki.vg/Server_List_Ping#Status_Response).
/// Useful fields include: `version.name`, `players.online/players.max`, `players.sample[x].name/.id`,
/// `description`, `favicon` (optional, b64 png format)
/// `players.sample` is also an optional field, and will be omitted from the response
/// if there are no online players (`players.online==0`)
#[post("/job/{id}")]
async fn post_job(path: Path<u32>, json: String, state: Data<ServerState>) -> Result<impl Responder> {
    let id = path.into_inner();

    if json.is_empty() {
        return Ok(HttpResponse::BadRequest().body("Invalid JSON data received (empty)"));
    }

    // Parse json
    let json: Value = serde_json::from_str(&json).map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    let status = json["status"].as_str()
        .ok_or_else(|| error::ErrorBadRequest("Missing 'status' field"))?;
    let ip: Ipv4Addr = json["ip"].as_str()
        .ok_or_else(|| error::ErrorBadRequest("Missing `ip` field"))?
        .parse().map_err(|e: AddrParseError| error::ErrorBadRequest(e.to_string()))?;
    let response = json["response"].clone();

    if status.to_lowercase() != "up" {
        // Remove job from outstanding list (if already in list)
        let mut outstanding = state.outstanding_client_jobs.lock().unwrap();
        if let Some(idx) = outstanding.iter().position(|x| x.id == id && x.ip == ip) {
            outstanding.remove(idx);
        };
        return Ok(HttpResponse::Ok().finish());
    }

    // Server is up, expect response
    if response.is_null() {
        // Empty response when status is up, invalid state
        return Err(error::ErrorBadRequest("No response provided while status is 'up'"));
    }

    // Get fields
    let _version = response["version"]["name"].as_str();
    let _description = response["description"].as_str();
    let _favicon = response["favicon"].as_str();
    let _players_connected = response["players"]["online"].as_u64();
    let _players_max = response["players"]["max"].as_u64();
    // TODO Implement saving to DB when working on MC-15

    // Remove from outstanding job list
    let mut outstanding = state.outstanding_client_jobs.lock().unwrap();
    if let Some(idx) = outstanding.iter().position(|x| x.id == id && x.ip == ip) {
        outstanding.remove(idx);
    };

    Ok(HttpResponse::Ok().finish())
}
