use std::ops::Deref;
use actix_web::{get, HttpResponse, Responder, Scope};
use actix_web::web::{Data, scope};
use crate::ServerState;

pub fn get_info_scope() -> Scope {
    scope("/info")
        .service(get_valid_ips)
}

#[get("/ips")]
async fn get_valid_ips(state: Data<ServerState>) -> impl Responder {
    let ips = state.valid_ips.lock().unwrap();
    HttpResponse::Ok().json(ips.deref())
}