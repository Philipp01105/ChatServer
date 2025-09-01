use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::user::User;
use bcrypt::{hash, verify, DEFAULT_COST};
use regex::Regex;

#[derive(Debug, Serialize, Deserialize, Default)]
struct UserDatabase {
    users: HashMap<String, String>,
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
        self.validate_username(username)?;
        self.validate_password(password)?;

        if self.database.users.contains_key(username) {
            return Err("Username already exists".to_string());
        }

        let hashed_password = hash(password, DEFAULT_COST)
            .map_err(|_| "Failed to hash password".to_string())?;
        
        self.database.users.insert(username.to_string(), hashed_password);
        self.save_database()?;

        Ok(User::new(username.to_string()))
    }

    pub fn login(&self, username: &str, password: &str) -> Result<User, String> {
        self.validate_username(username)?;
        self.validate_password(password)?;
        
        match self.database.users.get(username) {
            Some(stored_hash) => {
                if verify(password, stored_hash)
                    .map_err(|_| "Password verification failed".to_string())? {
                    Ok(User::new(username.to_string()))
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

        // Write to a temporary file first, then rename for atomic operation
        let temp_file = format!("{}.tmp", self.file_path);
        
        fs::write(&temp_file, json)
            .map_err(|e| format!("Failed to write temporary database file: {}", e))?;
            
        // Atomic rename (moves temp file to final location)
        fs::rename(&temp_file, &self.file_path)
            .map_err(|e| format!("Failed to rename database file: {}", e))?;

        Ok(())
    }
    
    fn validate_username(&self, username: &str) -> Result<(), String> {
        if username.is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        
        if username.len() > 32 {
            return Err("Username too long (max 32 characters)".to_string());
        }
        
        let username_regex = Regex::new(r"^[a-zA-Z0-9_-]+$")
            .map_err(|_| "Invalid username format".to_string())?;
            
        if !username_regex.is_match(username) {
            return Err("Username can only contain letters, numbers, underscores, and hyphens".to_string());
        }
        
        Ok(())
    }
    
    fn validate_password(&self, password: &str) -> Result<(), String> {
        if password.is_empty() {
            return Err("Password cannot be empty".to_string());
        }
        
        if password.len() < 8 {
            return Err("Password must be at least 8 characters long".to_string());
        }
        
        if password.len() > 128 {
            return Err("Password too long (max 128 characters)".to_string());
        }
        
        Ok(())
    }
}
