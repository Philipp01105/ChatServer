use std::collections::HashMap;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChannelType {
    Text,
    Voice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub channel_type: ChannelType,
    pub users: Vec<String>,
}

impl Channel {
    pub fn new(name: String, channel_type: ChannelType) -> Self {
        Channel {
            name,
            channel_type,
            users: Vec::new(),
        }
    }
}

pub struct ChannelManager {
    channels: HashMap<String, Channel>,
    config_file: String,
}

impl ChannelManager {
    pub fn new() -> Self {
        let mut manager = ChannelManager {
            channels: HashMap::new(),
            config_file: "channels.json".to_string(),
        };
        
        manager.load_channels().unwrap_or_else(|e| {
            eprintln!("Failed to load channels: {}", e);
        });
        
        manager
    }
    
    pub fn new_with_config(config_file: &str) -> Self {
        let mut manager = ChannelManager {
            channels: HashMap::new(),
            config_file: config_file.to_string(),
        };
        
        manager.load_channels().unwrap_or_else(|e| {
            eprintln!("Failed to load channels: {}", e);
        });
        
        manager
    }

    pub fn create_channel(&mut self, name: &str, channel_type: ChannelType) -> bool {
        if self.channels.contains_key(name) {
            return false;
        }

        self.channels.insert(
            name.to_string(),
            Channel::new(name.to_string(), channel_type)
        );
        
        self.save_channels().unwrap_or_else(|e| {
            eprintln!("Failed to save channels: {}", e);
        });
        
        true
    }

    pub fn channel_exists(&self, name: &str) -> bool {
        self.channels.contains_key(name)
    }

    pub fn get_channel(&self, name: &str) -> Option<&Channel> {
        self.channels.get(name)
    }

    pub fn join_channel(&mut self, channel_name: &str, username: String) {
        if let Some(channel) = self.channels.get_mut(channel_name) {
            if !channel.users.contains(&username) {
                channel.users.push(username);
            }
        }
    }

    pub fn leave_channel(&mut self, channel_name: &str, username: &str) {
        if let Some(channel) = self.channels.get_mut(channel_name) {
            channel.users.retain(|u| u != username);
        }
    }

    pub fn leave_all_channels(&mut self, username: &str) {
        for channel in self.channels.values_mut() {
            channel.users.retain(|u| u != username);
        }
    }

    pub fn list_channels(&self) -> Vec<(String, ChannelType, usize)> {
        self.channels.values()
            .map(|ch| (ch.name.clone(), ch.channel_type.clone(), ch.users.len()))
            .collect()
    }
    
    fn load_channels(&mut self) -> Result<(), String> {
        if !std::path::Path::new(&self.config_file).exists() {
            // Create default channels if file doesn't exist
            self.channels.insert("general".to_string(), Channel::new("general".to_string(), ChannelType::Text));
            self.channels.insert("random".to_string(), Channel::new("random".to_string(), ChannelType::Text));
            self.channels.insert("voice-lobby".to_string(), Channel::new("voice-lobby".to_string(), ChannelType::Voice));
            self.channels.insert("gaming".to_string(), Channel::new("gaming".to_string(), ChannelType::Voice));
            return self.save_channels();
        }
        
        // Simple file read without locking to avoid conflicts
        let content = fs::read_to_string(&self.config_file)
            .map_err(|e| format!("Failed to read channels file: {}", e))?;
            
        let channels: HashMap<String, Channel> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse channels file: {}", e))?;
        
        self.channels = channels;
        Ok(())
    }
    
    fn save_channels(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.channels)
            .map_err(|e| format!("Failed to serialize channels: {}", e))?;
            
        // Write to a temporary file first, then rename for atomic operation
        let temp_file = format!("{}.tmp", self.config_file);
        
        fs::write(&temp_file, json)
            .map_err(|e| format!("Failed to write temporary channels file: {}", e))?;
            
        // Atomic rename (moves temp file to final location)
        fs::rename(&temp_file, &self.config_file)
            .map_err(|e| format!("Failed to rename channels file: {}", e))?;
            
        Ok(())
    }
}
