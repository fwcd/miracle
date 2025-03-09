use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Context, Result};
use dashmap::DashMap;
use lighthouse_protocol::{DirectoryTree, Value};
use tokio::sync::mpsc;

use crate::model::{Directory, Node};

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

    fn split_lookup<'a, 'b>(tree: &'a mut Directory, path: &'b [String]) -> Result<(&'a mut Directory, &'b str)> {
        assert!(!path.is_empty());

        let parent_path = &path[..path.len() - 1];
        let name = &path[path.len() - 1];

        let parent: &mut Directory = {
            if parent_path.is_empty() {
                tree
            } else {
                match tree.get_path_mut(parent_path).ok_or_else(|| anyhow!("Parent path does not exist: {parent_path:?}"))? {
                    Node::Resource(_) => bail!("Parent path points to a resource: {parent_path:?}"),
                    Node::Directory(directory) => directory,
                } 
            }
        };

        Ok((parent, name))
    }

    /// Checks whether the given path exists.
    pub fn exists(&self, path: &[String]) -> Result<bool> {
        let tree = self.tree.lock().unwrap();
        Ok(tree.get_path(path).is_some())
    }

    /// Inserts or updates the given node at the given path.
    pub fn insert(&self, path: &[String], node: Node) -> Result<()> {
        let mut tree = self.tree.lock().unwrap();
        let (parent, name) = Self::split_lookup(&mut tree, path)?;
        parent.insert(name.into(), node);
        Ok(())
    }

    /// Removes the node at the given path.
    pub fn remove(&self, path: &[String]) -> Result<()> {
        let mut tree = self.tree.lock().unwrap();
        let (parent, name) = Self::split_lookup(&mut tree, path)?;
        parent.remove(name);
        Ok(())
    }

    /// Fetches the resource at the given path.
    pub fn get(&self, path: &[String]) -> Result<Value> {
        let tree = self.tree.lock().unwrap();
        Ok(tree.get_path(path)
            .context("Could not find path")?
            .as_resource()
            .context("Path is not a resource")?
            .value()
            .clone())
    }

    /// Lists the tree under the given path.
    pub fn list_tree(&self, path: &[String]) -> Result<DirectoryTree> {
        let tree = self.tree.lock().unwrap();
        Ok(tree.get_path(path)
            .context("Could not find path")?
            .as_directory()
            .context("Path is not a directory")?
            .list_tree())
    }
}
