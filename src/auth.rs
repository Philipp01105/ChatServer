use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::user::User;

#[derive(Debug, Serialize, Deserialize)]
struct UserDatabase {
    users: HashMap<String, String>,
}

impl Default for UserDatabase {
    fn default() -> Self {
        UserDatabase {
            users: HashMap::new(),
        }
    }
}

pub struct AuthManager {
    file_path: String,
    database: UserDatabase,
}

impl AuthManager {
    pub fn new(file_path: &str) -> Self {
        let database = if Path::new(file_path).exists() {
            let content = fs::read_to_string(file_path)
                .expect("Failed to read user database");
            serde_json::from_str(&content)
                .unwrap_or_default()
        } else {
            UserDatabase::default()
        };

        AuthManager {
            file_path: file_path.to_string(),
            database,
        }
    }

    pub fn register(&mut self, username: &str, password: &str) -> Result<User, String> {
        if username.is_empty() {
            return Err("Username cannot be empty".to_string());
        }

        if password.is_empty() {
            return Err("Password cannot be empty".to_string());
        }

        if self.database.users.contains_key(username) {
            return Err("Username already exists".to_string());
        }

        self.database.users.insert(username.to_string(), password.to_string());
        self.save_database()?;

        Ok(User::new(username.to_string(), password.to_string()))
    }

    pub fn login(&self, username: &str, password: &str) -> Result<User, String> {
        match self.database.users.get(username) {
            Some(stored_password) => {
                if stored_password == password {
                    Ok(User::new(username.to_string(), password.to_string()))
                } else {
                    Err("Invalid password".to_string())
                }
            }
            None => Err("Username not found".to_string()),
        }
    }

    fn save_database(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.database)
            .map_err(|e| format!("Failed to serialize database: {}", e))?;

        fs::write(&self.file_path, json)
            .map_err(|e| format!("Failed to write database: {}", e))?;

        Ok(())
    }
}