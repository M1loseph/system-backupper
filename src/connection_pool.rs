use anyhow::{Error, Result};
use rusqlite::Connection;
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
};

pub struct ConnectionPool {
    connection_pool: HashMap<u32, Mutex<Connection>>,
}

impl ConnectionPool {
    pub fn new(sqlite_path: &str, connection_pool_size: u32) -> Result<ConnectionPool> {
        let sqlite_path = Path::new(&sqlite_path);
        if let Some(parent) = sqlite_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connections = (0..connection_pool_size)
            .into_iter()
            .map(|_| Connection::open(&sqlite_path).map_err(|err| Error::from(err)))
            .collect::<Result<Vec<Connection>>>()?;

        let pool = connections
            .into_iter()
            .enumerate()
            .map(|(i, conn)| (i as u32, Mutex::new(conn)))
            .collect();
        Ok(ConnectionPool {
            connection_pool: pool,
        })
    }

    pub fn get_random_connection(&'_ self) -> MutexGuard<'_, Connection> {
        // Unwrapping here seems ok. It is said on reddit and rust forum that accessing poisoned data should
        // lead to panick.
        //
        // https://users.rust-lang.org/t/should-i-unwrap-a-mutex-lock/61519
        // https://www.reddit.com/r/rust/comments/xy2rkl/whats_the_best_way_to_avoid_an_unwrap_of_a_mutex/
        let index = rand::random::<u32>() % self.connection_pool.len() as u32;
        self.connection_pool[&index].lock().unwrap()
    }
}
