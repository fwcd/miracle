use std::sync::Arc;

use tokio::sync::{Mutex, MutexGuard};

use crate::model::Directory;

#[derive(Clone)]
pub struct State {
    tree: Arc<Mutex<Directory>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            tree: Arc::new(Mutex::new(Directory::new())),
        }
    }

    pub async fn lock_tree(&self) -> MutexGuard<Directory> {
        self.tree.lock().await
    }
}
