use std::collections::HashMap;

use lighthouse_protocol::Value;

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
}

impl Directory {
    pub fn new() -> Self {
        Directory {
            children: HashMap::new(),
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
