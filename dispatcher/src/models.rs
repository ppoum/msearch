use std::error::Error;
use custom_error::custom_error;
use diesel::prelude::*;
use ipnet::IpNet;
use serde_json::Value;
use uuid::Uuid;
use crate::DbConnection;
use crate::schema::{scan, player, player_scan};

// ERRORS
custom_error! {pub DBError
    PlayerCreationError = "There was an error creating a new player in the database."}

//
// SCAN
//

#[derive(Queryable, Identifiable)]
#[diesel(table_name = scan)]
pub struct Scan {
    pub id: i32,
    pub ip: IpNet,
    pub version: Option<String>,
    pub online_count: Option<i32>,
    pub max_count: Option<i32>,
    pub description: Option<String>,
    pub favicon: Option<String>
}

#[derive(Insertable)]
#[diesel(table_name = scan)]
pub struct NewScan {
    pub ip: IpNet,
    pub version: Option<String>,
    pub online_count: Option<i32>,
    pub max_count: Option<i32>,
    pub description: Option<String>,
    pub favicon: Option<String>
}

impl NewScan {
    pub fn save_to_db(&self, conn: &mut DbConnection) -> QueryResult<Scan> {
        diesel::insert_into(scan::table).values(self).get_result::<Scan>(conn)
    }
}

//
// PLAYER
//

#[derive(Queryable, Identifiable)]
#[diesel(table_name = player)]
pub struct Player {
    pub id: i32,
    pub username: String,
    // Official player UUID from Mojang (through PlayerDB), None if no player w/ username
    pub player_uuid: Option<Uuid>
}

impl Player {
    /// Communicates with [PlayerDB](https://playerdb.co/) to retrieve player's username
    /// based on the UUID of the current struct object.
    ///
    /// Returns `Some(username)` if successful, `None` otherwise
    pub fn update_username(&self, conn: &mut DbConnection) -> Option<String> {
        use crate::schema::player::dsl::*;
        if self.player_uuid.is_none() {
            panic!("Tried to update the username of player {}, but player has no UUID associated", self.username);
        }

        let url = format!("https://playerdb.co/api/player/minecraft/{}", self.player_uuid.unwrap());
        let result = match reqwest::blocking::get(url) {
            Ok(r) => r,
            Err(_) => return None
        };
        let result = match result.text() {
            Ok(s) => s,
            Err(_) => return None
        };

        let result: Value = serde_json::from_str(&result).expect("Bad JSON response from PlayerDB");
        let status = result["success"].as_bool().expect("Bad JSON response from PlayerDB");

        if !status {
            panic!("UUID no longer exists according to PlayerDB");
        }
        let new_username = result["data"]["player"]["username"].as_str().expect("Bad JSON response from PlayerDB");
        match diesel::update(self).set(username.eq(new_username)).execute(conn) {
            Ok(_) => Some(String::from(new_username)),
            Err(_) => None
        }
    }

    /// Creates a player in the database. Player username and UUID *MUST* match according to Mojang.
    /// Do not input a potentially fake/offline UUID as a parameter.
    ///
    /// If a UUID is provided, the database is queried according to that UUID to find a row. If present,
    /// the row's name will be updated to the provided username.
    ///
    /// If no UUID is provided (this means this is a fake account that doesn't exist), the database
    /// is queried with the provided player username. If no row exists with that name, a new entry
    /// will be created, and the UUID will be set to null.
    pub fn create_if_not_exist(name: String, uuid: Option<Uuid>, conn: &mut DbConnection) -> Result<Player, DBError> {
        use crate::schema::player::dsl::*;

        // Get player w/ UUID if present, otherwise get w/ username
        let result = match uuid {
            Some(uuid) => player.filter(player_uuid.eq(uuid)).first::<Player>(conn).optional(),
            None => player.filter(username.eq(name.clone())).first::<Player>(conn).optional()
        };


        let result = match result {
            Ok(x) => x,
            Err(_) => todo!()  // Error querying database
        };

        match result {
            Some(p) => {
                // Player already exists, update username if needed
                if p.username != name {
                    match diesel::update(&p).set(username.eq(name)).get_result::<Player>(conn) {
                        Ok(x) => Ok(x),
                        Err(_) => Err(DBError::PlayerCreationError)
                    }
                } else {
                    Ok(p)
                }
            }
            None => {
                // Player doesn't exist, create
                let x = NewPlayer { username: name, player_uuid: uuid };
                match x.save_to_db(conn) {
                    Ok(x) => Ok(x),
                    Err(_) => Err(DBError::PlayerCreationError)
                }
            }
        }
    }

    pub fn query_playerdb(username: &str) -> Result<Option<Uuid>, Box<dyn Error>> {
        let url = format!("https://playerdb.co/api/player/minecraft/{}", username);

        // Parse response
        let result = reqwest::blocking::get(url)?;
        let result = result.text()?;

        let result: Value = serde_json::from_str(&result).expect("Bad JSON response from PlayerDB");

        // Extract data
        let status = result["success"].as_bool().expect("Bad JSON response from PlayerDB");

        if !status {
            return Ok(None);
        }

        let uuid = result["data"]["player"]["id"].as_str().expect("Bad JSON response from PlayerDB");
        Ok(Some(Uuid::parse_str(uuid).expect("Bad JSON response from PlayerDB")))
    }
}

#[derive(Insertable)]
#[diesel(table_name = player)]
pub struct NewPlayer {
    pub username: String,
    pub player_uuid: Option<Uuid>
}

impl NewPlayer {
    pub fn save_to_db(&self, conn: &mut DbConnection) -> QueryResult<Player> {
        diesel::insert_into(player::table).values(self).get_result::<Player>(conn)
    }
}


//
// PLAYER-SCAN RELATION
//

#[derive(Queryable, Associations)]
#[diesel(table_name = player_scan)]
#[diesel(belongs_to(Player))]
#[diesel(belongs_to(Scan))]
pub struct PlayerScan {
    // Player UUID gotten from that specific scan. Can be used to detect offline mode logins
    pub player_scan_uuid: Uuid,
    pub player_id: i32,
    pub scan_id: i32
}

#[derive(Insertable)]
#[diesel(table_name = player_scan)]
pub struct NewPlayerScan {
    pub player_scan_uuid: Uuid,  // UUID gotten from specific scan
    pub player_id: i32,
    pub scan_id: i32
}

impl NewPlayerScan {
    pub fn save_to_db(&self, conn: &mut DbConnection) -> QueryResult<PlayerScan> {
        diesel::insert_into(player_scan::table)
            .values(self).get_result::<PlayerScan>(conn)
    }
}