use std::collections::HashMap;

pub struct VoiceSession {
    pub username: String,
    pub channel: String,
    pub is_muted: bool,
    pub is_deafened: bool,
}

pub struct VoiceChannelManager {
    sessions: HashMap<String, VoiceSession>,
}

impl VoiceChannelManager {
    pub fn new() -> Self {
        VoiceChannelManager {
            sessions: HashMap::new(),
        }
    }

    pub fn join_voice_channel(&mut self, username: String, channel: String) {
        self.sessions.insert(username.clone(), VoiceSession {
            username: username.clone(),
            channel,
            is_muted: false,
            is_deafened: false,
        });
    }

    pub fn leave_voice_channel(&mut self, username: &str) -> bool {
        self.sessions.remove(username).is_some()
    }

    pub fn toggle_mute(&mut self, username: &str) -> Option<bool> {
        self.sessions.get_mut(username).map(|session| {
            session.is_muted = !session.is_muted;
            session.is_muted
        })
    }

    pub fn toggle_deafen(&mut self, username: &str) -> Option<bool> {
        self.sessions.get_mut(username).map(|session| {
            session.is_deafened = !session.is_deafened;
            if session.is_deafened {
                session.is_muted = true;
            }
            session.is_deafened
        })
    }

    pub fn get_channel_users(&self, channel: &str) -> Vec<String> {
        self.sessions.values()
            .filter(|s| s.channel == channel)
            .map(|s| s.username.clone())
            .collect()
    }
}

