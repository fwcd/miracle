use std::sync::Arc;

use dashmap::DashMap;
use lighthouse_protocol::Value;
use tokio::sync::{mpsc, Mutex, MutexGuard};

use crate::model::Directory;

#[derive(Clone)]
pub struct State {
    tree: Arc<Mutex<Directory>>,
    streams: Arc<DashMap<Vec<String>, Vec<mpsc::Sender<Value>>>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            tree: Arc::new(Mutex::new(Directory::new())),
            streams: Arc::new(DashMap::new()),
        }
    }

    pub async fn lock_tree(&self) -> MutexGuard<Directory> {
        self.tree.lock().await
    }

    // TODO: Replace lock_tree with abstraction layer for CRUD, parent, directory lookup and notify streams
}
