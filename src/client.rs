use std::net::TcpStream;
use crate::user::User;
use uuid::Uuid;

#[derive(Debug)]
pub struct Client {
    pub id: Uuid,
    pub stream: TcpStream,
    pub user: User,
    pub current_channel: Option<String>,
}

impl Client {
    pub fn new(stream: TcpStream, user: User) -> Result<Self, std::io::Error> {
        Ok(Client {
            id: Uuid::new_v4(),
            stream,
            user,
            current_channel: Some("general".to_string()),
        })
    }
    
    pub fn try_clone(&self) -> Result<Self, std::io::Error> {
        Ok(Self {
            id: self.id,
            stream: self.stream.try_clone()?,
            user: self.user.clone(),
            current_channel: self.current_channel.clone(),
        })
    }
}

