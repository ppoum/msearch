use std::net::{AddrParseError, Ipv4Addr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;
use actix_web::{HttpResponse, Scope, Result, Responder, error, get, post, web};
use actix_web::web::{Data, Path, scope};
use ipnet::{IpNet, Ipv4Net};
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::SerializeStruct;
use serde_json::Value;
use uuid::Uuid;
use crate::{DbPool, ServerState};
use crate::models::{NewPlayerScan, Player};
use crate::models::NewScan;

static JOB_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Deserialize, Copy, Clone)]
pub struct ClientJob {
    pub id: u32,
    pub ip: Ipv4Addr,
    pub creation_time: SystemTime,
}

// Default used when deserializing w/ missing fields
impl Default for ClientJob {
    fn default() -> Self {
        ClientJob {
            id: 0,
            ip: Ipv4Addr::new(0, 0, 0, 0),
            creation_time: SystemTime::now(),
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
                creation_time: SystemTime::now(),
            };
            // Add job to outstanding list
            let mut outstanding = state.outstanding_client_jobs.lock().unwrap();
            outstanding.push_back(job);

            Ok(HttpResponse::Ok().json(job))
        }
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
async fn post_job(path: Path<u32>, json: String, state: Data<ServerState>, pool: Data<DbPool>) -> Result<impl Responder> {
    // This function is truly horrific, rewrite when possible (refactor validation into separate function?)

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

    // If server isn't up, no need to save to db, just remove from outstanding job list
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

    // Remove from outstanding job list
    {
        let mut outstanding = state.outstanding_client_jobs.lock().unwrap();
        if let Some(idx) = outstanding.iter().position(|x| x.id == id && x.ip == ip) {
            outstanding.remove(idx);
        };
    }

    // Save retrieved values into DB
    // DB isn't async, run in block
    let success = web::block(move || {
        let mut conn = pool.get()
            .expect("Could not obtain database connection.");


        // Create Scan struct instance
        // Description is a JSON object, serialize to string before saving to db
        let desc = response.get("description")
            .map(|x| x.to_string());
        let new_scan = NewScan {
            ip: IpNet::V4(Ipv4Net::from(ip)),
            version: response["version"]["name"].as_str().map(String::from),
            online_count: response["players"]["online"].as_u64().map(|x| x as i32),
            max_count: response["players"]["max"].as_u64().map(|x| x as i32),
            description: desc,
            favicon: response["favicon"].as_str().map(String::from),
        };
        let new_scan = match new_scan.save_to_db(&mut conn) {
            Ok(s) => s,
            Err(_) => return false
        };

        let players = response["players"]["sample"].as_array();

        if players.is_none() {
            return true;
        }

        for u in players.unwrap() {
            let player_name = u["name"].as_str().unwrap();
            match Uuid::parse_str(u["id"].as_str().unwrap()) {
                Ok(u) => {
                    // Workflow:
                    // Query PlayerDB with username to get UUID
                    // If PlayerDB UUID matches server UUID:
                    //   Valid online player, save in DB, updating username of entry if needed
                    // If username isn't a valid MC account name:
                    //   Create DB entry with username and null UUID
                    // If username exists with different UUID:
                    //   Create DB entry w/ real name and UUID, assoc with server UUID (wrong/offline UUID)

                    let db_uuid = match Player::query_playerdb(player_name) {
                        Ok(x) => x,
                        Err(_) => return false
                    };

                    let db_player = match db_uuid {
                        Some(db_uuid) => {
                            // Username is real, either UUIDs match (online) or not (offline)
                            // In both cases, create user in db if needed
                            match Player::create_if_not_exist(String::from(player_name), Some(db_uuid), &mut conn) {
                                Ok(p) => p,
                                Err(_) => return false
                            }
                        }
                        None => {
                            // Fake username, create user in db with no UUID (if needed)
                            match Player::create_if_not_exist(String::from(player_name), None, &mut conn) {
                                Ok(p) => p,
                                Err(_) => return false
                            }
                        }
                    };

                    let new_playerscan = NewPlayerScan {
                        player_id: db_player.id,
                        scan_id: new_scan.id,
                        player_scan_uuid: u
                    };
                    match new_playerscan.save_to_db(&mut conn) {
                        Ok(_) => {}
                        Err(_) => return false
                    }
                }
                Err(_) => return false
            }
        }
        true
    }).await.map_err(|e| error::ErrorInternalServerError(e.to_string()))?;

    if success {
        Ok(HttpResponse::Ok().finish())
    } else {
        Err(error::ErrorInternalServerError("Could not successfully save scan to database"))
    }
}
