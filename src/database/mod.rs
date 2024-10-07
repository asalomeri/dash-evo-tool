mod contested_names;
mod contracts;
mod identities;
mod initialization;

use dash_sdk::dpp::identity::accessors::IdentityGettersV0;
use dash_sdk::dpp::serialization::PlatformSerializable;
use rusqlite::{Connection, Params, Statement};
use std::sync::Mutex;

#[derive(Debug)]
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn execute<P: Params>(&self, sql: &str, params: P) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute(sql, params)
    }
}
