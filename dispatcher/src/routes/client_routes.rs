use rocket::response::status;
use rocket::{Route, State};
use crate::ServerState;

pub fn get_all_routes() -> Vec<Route> {
    routes![get_job]
}

#[get("/job")]
fn get_job(state: &State<ServerState>) -> Result<String, status::NotFound<&str>> {
    let mut valid_ips = state.valid_ips.lock().unwrap();

    match valid_ips.pop_front() {
        Some(ip) => {
            Ok(ip.to_string())
        },
        None => Err(status::NotFound("No job available"))
    }
}