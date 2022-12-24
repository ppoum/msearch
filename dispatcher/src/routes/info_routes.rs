use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::ops::Deref;
use rocket::serde::json::Json;
use rocket::{Route, State};
use crate::ServerState;

pub fn get_all_routes() -> Vec<Route> {
    routes![get_valid_ips]
}

#[get("/ips")]
fn get_valid_ips(state: &State<ServerState>) -> Json<VecDeque<Ipv4Addr>> {
    let ips = state.valid_ips.lock().unwrap();
    Json(ips.deref().clone())
}