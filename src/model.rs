use std::collections::HashMap;

use lighthouse_protocol::{DirectoryTree, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Directory {
    children: HashMap<String, Node>,
}

#[derive(Debug, Clone, PartialEq)]
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

    pub fn contains(&self, name: &str) -> bool {
        self.children.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) {
        self.children.remove(name);
    }

    pub fn insert(&mut self, name: String, node: Node) {
        self.children.insert(name, node);
    }

    #[allow(unused)]
    pub fn get_path(&self, path: &[String]) -> Option<&Node> {
        path.first().and_then(|first| {
            let is_leaf = path.len() == 1;
            let child = self.get(first)?;
            if is_leaf {
                Some(child)
            } else {
                match child {
                    Node::Resource(_) => None,
                    Node::Directory(child_dir) => child_dir.get_path(&path[1..]),
                }  
            }
        })
    }

    pub fn get_path_mut(&mut self, path: &[String]) -> Option<&mut Node> {
        path.first().and_then(|first| {
            let is_leaf = path.len() == 1;
            let child = self.get_mut(first)?;
            if is_leaf {
                Some(child)
            } else {
                match child {
                    Node::Resource(_) => None,
                    Node::Directory(child_dir) => child_dir.get_path_mut(&path[1..]),
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
