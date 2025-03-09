use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use dashmap::DashMap;
use futures::{channel::mpsc, SinkExt, Stream};
use lighthouse_protocol::{DirectoryTree, Value};
use stream_guard::GuardStreamExt;
use tokio::task::JoinSet;

use crate::model::{Directory, Node, Resource};

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

        let parent: &mut Directory = tree.descendant_directory_mut(parent_path)
            .with_context(|| format!("Parent path {parent_path:?} does not point to a directory"))?;

        Ok((parent, name))
    }

    /// Notifies the streams of the given path about the given value.
    async fn notify_streams(&self, path: &[String], value: &Value) -> Result<()> {
        let mut set = JoinSet::new();

        if let Some(streams) = self.streams.get(path) {
            for mut stream in streams.clone() {
                let value = value.clone();
                set.spawn_local(async move {
                    stream.send(value).await
                });
            }
        }

        set.join_all().await.into_iter().collect::<Result<(), _>>()?;
        Ok(())
    }

    /// Checks whether the given path exists.
    pub fn exists(&self, path: &[String]) -> Result<bool> {
        let tree = self.tree.lock().unwrap();
        Ok(tree.descendant(path).is_some())
    }

    /// Inserts the given resource at the given path.
    pub async fn insert_resource(&self, path: &[String], resource: Resource) -> Result<()> {
        {
            let mut tree = self.tree.lock().unwrap();
            let (parent, name) = Self::split_lookup(&mut tree, path)?;
            parent.insert(name.into(), Node::Resource(resource.clone()));
        }
        self.notify_streams(path, resource.value()).await?;
        Ok(())
    }

    /// Insert the given directory at the given path (and overrides anything old).
    pub async fn insert_directory(&self, path: &[String], directory: Directory) -> Result<()> {
        let mut tree = self.tree.lock().unwrap();
        let (parent, name) = Self::split_lookup(&mut tree, path)?;
        parent.insert(name.into(), Node::Directory(directory));
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
        Ok(tree.descendant(path)
            .with_context(|| format!("Could not find path {path:?}"))?
            .as_resource()
            .with_context(|| format!("Path {path:?} is not a resource"))?
            .value()
            .clone())
    }

    /// Lists the tree under the given path.
    pub fn list_tree(&self, path: &[String]) -> Result<DirectoryTree> {
        let tree = self.tree.lock().unwrap();
        Ok(tree.descendant_directory(path)
            .with_context(|| format!("Path {path:?} is not a directory"))?
            .list_tree())
    }

    /// Starts a stream of the given resource that is automatically stopped once dropped.
    pub fn stream(&self, path: &[String]) -> Result<impl Stream<Item = Value>> {
        let (tx, rx) = mpsc::channel(4);

        let streams = Arc::clone(&self.streams);
        let path = path.to_vec();

        if !streams.contains_key(&path) {
            streams.insert(path.clone(), Vec::new());
        }

        streams.get_mut(&path).unwrap().push(tx);

        Ok(rx.guard(move || {
            streams.remove(&path);
        }))
    }
}
