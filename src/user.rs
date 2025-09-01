use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub password: String,
}

impl User {
    pub fn new(name: String, password: String) -> Self {
        User { name, password }
    }
}