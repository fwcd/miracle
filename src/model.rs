use std::collections::HashMap;

use lighthouse_protocol::{DirectoryTree, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Directory {
    // TODO: Profile whether we'd want to use a DashMap here too and make all
    // methods take &self, so we could avoid the mutex in State.
    children: HashMap<String, Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Resource(Resource),
    Directory(Directory),
}

impl Resource {
    pub fn new() -> Self {
        Self {
            value: Value::Nil,
        }
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

impl Directory {
    pub fn new() -> Self {
        Directory {
            children: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Node> {
        self.children.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.children.get_mut(name)
    }

    pub fn remove(&mut self, name: &str) {
        self.children.remove(name);
    }

    pub fn insert(&mut self, name: String, node: Node) {
        self.children.insert(name, node);
    }

    pub fn descendant_directory(&self, path: &[String]) -> Option<&Directory> {
        if path.is_empty() {
            Some(self)
        } else {
            self.descendant(path)?.as_directory()
        }
    }

    pub fn descendant_directory_mut(&mut self, path: &[String]) -> Option<&mut Directory> {
        if path.is_empty() {
            Some(self)
        } else {
            self.descendant_mut(path)?.as_directory_mut()
        }
    }

    pub fn descendant(&self, path: &[String]) -> Option<&Node> {
        assert!(!path.is_empty());
        path.first().and_then(|first| {
            let is_leaf = path.len() == 1;
            let child = self.get(first)?;
            if is_leaf {
                Some(child)
            } else {
                match child {
                    Node::Resource(_) => None,
                    Node::Directory(child_dir) => child_dir.descendant(&path[1..]),
                }  
            }
        })
    }

    pub fn descendant_mut(&mut self, path: &[String]) -> Option<&mut Node> {
        assert!(!path.is_empty());
        path.first().and_then(|first| {
            let is_leaf = path.len() == 1;
            let child = self.get_mut(first)?;
            if is_leaf {
                Some(child)
            } else {
                match child {
                    Node::Resource(_) => None,
                    Node::Directory(child_dir) => child_dir.descendant_mut(&path[1..]),
                }  
            }
        })
    }

    pub fn list_tree(&self) -> DirectoryTree {
        DirectoryTree {
            entries: self.children.iter()
                .map(|(name, child)| (name.clone(), child.as_directory().map(|d| d.list_tree())))
                .collect()
        }
    }
}

impl Node {
    pub fn as_resource(&self) -> Option<&Resource> {
        if let Self::Resource(res) = self {
            Some(res)
        } else {
            None
        }
    }

    pub fn as_directory(&self) -> Option<&Directory> {
        if let Self::Directory(dir) = self {
            Some(dir)
        } else {
            None
        }
    }

    pub fn as_directory_mut(&mut self) -> Option<&mut Directory> {
        if let Self::Directory(dir) = self {
            Some(dir)
        } else {
            None
        }
    }
}

impl From<Value> for Resource {
    fn from(value: Value) -> Self {
        Self { value }
    }
}

macro_rules! impl_from_value {
    ($($tys:ty),*) => {
        $(impl From<$tys> for Resource {
            fn from(value: $tys) -> Self {
                Self::from(Value::from(value))
            }
        })*
    };
}

impl_from_value!(
    u8, u16, u32, u64, usize,
    i8, i16, i32, i64, isize,
    f32, f64, String, &str
);
