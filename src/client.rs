use std::net::TcpStream;
use crate::user::User;
pub struct Client {
    pub stream: TcpStream,
    pub user: User,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Self {
            stream: self.stream.try_clone().unwrap(),
            user: self.user.clone(),
        }
    }
}
