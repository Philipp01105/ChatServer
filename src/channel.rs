use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelType {
    Text,
    Voice,
}

#[derive(Debug, Clone)]
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
}

impl ChannelManager {
    pub fn new() -> Self {
        ChannelManager {
            channels: HashMap::new(),
        }
    }

    pub fn create_channel(&mut self, name: &str, channel_type: ChannelType) -> bool {
        if self.channels.contains_key(name) {
            return false;
        }

        self.channels.insert(
            name.to_string(),
            Channel::new(name.to_string(), channel_type)
        );
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
}
