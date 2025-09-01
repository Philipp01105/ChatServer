mod client;
mod user;
mod auth;
mod channel;
mod voice;

use crate::auth::AuthManager;
use crate::channel::{ChannelManager, ChannelType};
use crate::client::Client;
use crate::voice::VoiceChannelManager;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

struct Server {
    clients: Arc<Mutex<Vec<Client>>>,
    auth_manager: Arc<Mutex<AuthManager>>,
    channel_manager: Arc<Mutex<ChannelManager>>,
    voice_manager: Arc<Mutex<VoiceChannelManager>>,
}

impl Server {
    fn new() -> Self {
        let mut channel_manager = ChannelManager::new();

        channel_manager.create_channel("general", ChannelType::Text);
        channel_manager.create_channel("random", ChannelType::Text);
        channel_manager.create_channel("voice-lobby", ChannelType::Voice);
        channel_manager.create_channel("gaming", ChannelType::Voice);

        Server {
            clients: Arc::new(Mutex::new(Vec::new())),
            auth_manager: Arc::new(Mutex::new(AuthManager::new("users.json"))),
            channel_manager: Arc::new(Mutex::new(channel_manager)),
            voice_manager: Arc::new(Mutex::new(VoiceChannelManager::new())),
        }
    }
}

fn handle_client(mut stream: TcpStream, server: Arc<Server>) {
    let authenticated_user = match authenticate_client(&mut stream, &server.auth_manager) {
        Some(user) => user,
        None => {
            let _ = stream.write_all(b"Authentication failed. Disconnecting.\n");
            return;
        }
    };

    println!("User {} authenticated successfully", authenticated_user.name);

    let mut client = Client {
        stream: stream.try_clone().unwrap(),
        user: authenticated_user,
        current_channel: Some("general".to_string()),
    };

    show_channels(&mut client.stream, &server.channel_manager);

    {
        let mut channel_manager = server.channel_manager.lock().unwrap();
        channel_manager.join_channel("general", client.user.name.clone());
    }

    let client_addr = stream.peer_addr().unwrap();
    {
        let mut clients_guard = server.clients.lock().unwrap();
        clients_guard.push(client.clone());
    }

    broadcast_to_channel(&server.clients, &server.channel_manager,
                         "general",
                         &format!("*** {} joined the channel ***\n", client.user.name),
                         None);

    let help_msg = "\n=== Commands ===\n\
                   /channels - List all channels\n\
                   /join <channel> - Join a text channel\n\
                   /voice <channel> - Join a voice channel\n\
                   /leave - Leave current voice channel\n\
                   /create <name> text|voice - Create a new channel\n\
                   /users - List users in current channel\n\
                   /quit - Exit chat\n\
                   ================\n\n";
    let _ = stream.write_all(help_msg.as_bytes());

    let mut buffer = [0; 512];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[..n]).trim().to_string();

                if message.starts_with('/') {
                    handle_command(&mut stream, &server, &message, &client.user.name, client_addr);
                } else {
                    let (username, current_channel) = {
                        let clients_guard = server.clients.lock().unwrap();
                        clients_guard.iter()
                            .find(|c| c.stream.peer_addr().unwrap() == client_addr)
                            .map(|c| (c.user.name.clone(), c.current_channel.clone()))
                            .unwrap_or_else(|| ("Unknown".to_string(), Some("general".to_string())))
                    };

                    if let Some(channel) = current_channel {
                        let full_message = format!("[{}] {}: {}\n", channel, username, message);
                        broadcast_to_channel(&server.clients, &server.channel_manager,
                                             &channel, &full_message, Some(client_addr));
                    }
                }
            }
            Err(_) => break,
        }
    }

    let (username, current_channel) = {
        let mut clients_guard = server.clients.lock().unwrap();
        let user_info = clients_guard.iter()
            .find(|c| c.stream.peer_addr().unwrap() == client_addr)
            .map(|c| (c.user.name.clone(), c.current_channel.clone()));

        clients_guard.retain(|c| c.stream.peer_addr().unwrap() != client_addr);
        user_info.unwrap_or_else(|| ("Unknown".to_string(), None))
    };

    {
        let mut channel_manager = server.channel_manager.lock().unwrap();
        channel_manager.leave_all_channels(&username);
    }

    if let Some(channel) = current_channel {
        broadcast_to_channel(&server.clients, &server.channel_manager,
                             &channel,
                             &format!("*** {} left the channel ***\n", username),
                             None);
    }

    println!("User {} disconnected", username);
}

fn handle_command(stream: &mut TcpStream, server: &Arc<Server>, command: &str, username: &str, client_addr: std::net::SocketAddr) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    match parts[0] {
        "/channels" => {
            show_channels(stream, &server.channel_manager);
        }
        "/join" => {
            if parts.len() < 2 {
                let _ = stream.write_all(b"Usage: /join <channel_name>\n");
                return;
            }

            let channel_name = parts[1];
            let mut channel_manager = server.channel_manager.lock().unwrap();

            if channel_manager.channel_exists(channel_name) {
                let old_channel = {
                    let clients = server.clients.lock().unwrap();
                    clients.iter()
                        .find(|c| c.stream.peer_addr().unwrap() == client_addr)
                        .and_then(|c| c.current_channel.clone())
                };

                if let Some(old) = old_channel {
                    channel_manager.leave_channel(&old, username);
                    broadcast_to_channel(&server.clients, &server.channel_manager,
                                         &old,
                                         &format!("*** {} left the channel ***\n", username),
                                         None);
                }

                channel_manager.join_channel(channel_name, username.to_string());

                {
                    let mut clients = server.clients.lock().unwrap();
                    if let Some(client) = clients.iter_mut()
                        .find(|c| c.stream.peer_addr().unwrap() == client_addr) {
                        client.current_channel = Some(channel_name.to_string());
                    }
                }

                let _ = stream.write_all(format!("Joined channel: {}\n", channel_name).as_bytes());
                broadcast_to_channel(&server.clients, &server.channel_manager,
                                     channel_name,
                                     &format!("*** {} joined the channel ***\n", username),
                                     Some(client_addr));
            } else {
                let _ = stream.write_all(b"Channel does not exist\n");
            }
        }
        "/voice" => {
            if parts.len() < 2 {
                let _ = stream.write_all(b"Usage: /voice <channel_name>\n");
                return;
            }

            let channel_name = parts[1];
            let channel_manager = server.channel_manager.lock().unwrap();

            if let Some(channel) = channel_manager.get_channel(channel_name) {
                if channel.channel_type == ChannelType::Voice {
                    let mut voice_manager = server.voice_manager.lock().unwrap();
                    voice_manager.join_voice_channel(username.to_string(), channel_name.to_string());
                    let _ = stream.write_all(format!("Joined voice channel: {}\n", channel_name).as_bytes());
                    let _ = stream.write_all(b"Note: Voice streaming not implemented. This is a placeholder.\n");
                } else {
                    let _ = stream.write_all(b"That's not a voice channel\n");
                }
            } else {
                let _ = stream.write_all(b"Voice channel does not exist\n");
            }
        }
        "/leave" => {
            let mut voice_manager = server.voice_manager.lock().unwrap();
            if voice_manager.leave_voice_channel(username) {
                let _ = stream.write_all(b"Left voice channel\n");
            } else {
                let _ = stream.write_all(b"You're not in a voice channel\n");
            }
        }
        "/create" => {
            if parts.len() < 3 {
                let _ = stream.write_all(b"Usage: /create <name> text|voice\n");
                return;
            }

            let channel_name = parts[1];
            let channel_type = match parts[2] {
                "text" => ChannelType::Text,
                "voice" => ChannelType::Voice,
                _ => {
                    let _ = stream.write_all(b"Channel type must be 'text' or 'voice'\n");
                    return;
                }
            };

            let mut channel_manager = server.channel_manager.lock().unwrap();
            if channel_manager.create_channel(channel_name, channel_type) {
                let _ = stream.write_all(format!("Created {} channel: {}\n", parts[2], channel_name).as_bytes());
            } else {
                let _ = stream.write_all(b"Channel already exists\n");
            }
        }
        "/users" => {
            let current_channel = {
                let clients = server.clients.lock().unwrap();
                clients.iter()
                    .find(|c| c.stream.peer_addr().unwrap() == client_addr)
                    .and_then(|c| c.current_channel.clone())
            };

            if let Some(channel) = current_channel {
                let channel_manager = server.channel_manager.lock().unwrap();
                if let Some(ch) = channel_manager.get_channel(&channel) {
                    let users = ch.users.join(", ");
                    let _ = stream.write_all(format!("Users in {}: {}\n", channel, users).as_bytes());
                }
            }
        }
        _ => {
            let _ = stream.write_all(b"Unknown command. Type /help for commands.\n");
        }
    }
}

fn show_channels(stream: &mut TcpStream, channel_manager: &Arc<Mutex<ChannelManager>>) {
    let manager = channel_manager.lock().unwrap();
    let channels = manager.list_channels();

    let mut response = String::from("\n=== Available Channels ===\n");
    for (name, channel_type, user_count) in channels {
        let type_str = match channel_type {
            ChannelType::Text => "📝",
            ChannelType::Voice => "🔊",
        };
        response.push_str(&format!("{} {} ({} users)\n", type_str, name, user_count));
    }
    response.push_str("========================\n");

    let _ = stream.write_all(response.as_bytes());
}

fn broadcast_to_channel(clients: &Arc<Mutex<Vec<Client>>>,
                        channel_manager: &Arc<Mutex<ChannelManager>>,
                        channel_name: &str,
                        message: &str,
                        exclude_addr: Option<std::net::SocketAddr>) {
    let channel_users = {
        let manager = channel_manager.lock().unwrap();
        manager.get_channel(channel_name)
            .map(|ch| ch.users.clone())
            .unwrap_or_default()
    };

    let mut clients_guard = clients.lock().unwrap();
    clients_guard.retain_mut(|client| {
        if !channel_users.contains(&client.user.name) {
            return true;
        }

        if let Some(addr) = exclude_addr {
            if client.stream.peer_addr().unwrap() == addr {
                return true;
            }
        }

        match client.stream.write_all(message.as_bytes()) {
            Ok(_) => true,
            Err(_) => false,
        }
    });
}

fn authenticate_client(stream: &mut TcpStream, auth_manager: &Arc<Mutex<AuthManager>>) -> Option<user::User> {
    let _ = stream.write_all(b"Welcome to the chat server!\n");
    let _ = stream.write_all(b"1. Login\n2. Register\nChoose option (1 or 2): ");

    let mut buffer = [0; 512];
    let choice = match stream.read(&mut buffer) {
        Ok(n) => String::from_utf8_lossy(&buffer[..n]).trim().to_string(),
        Err(_) => return None,
    };

    match choice.as_str() {
        "1" => login_user(stream, auth_manager),
        "2" => register_user(stream, auth_manager),
        _ => {
            let _ = stream.write_all(b"Invalid choice.\n");
            None
        }
    }
}

fn login_user(stream: &mut TcpStream, auth_manager: &Arc<Mutex<AuthManager>>) -> Option<user::User> {
    let _ = stream.write_all(b"Username: ");
    let username = read_line(stream)?;

    let _ = stream.write_all(b"Password: ");
    let password = read_line(stream)?;

    let auth = auth_manager.lock().unwrap();
    match auth.login(&username, &password) {
        Ok(user) => {
            let _ = stream.write_all(b"Login successful!\n");
            Some(user)
        }
        Err(e) => {
            let _ = stream.write_all(format!("Login failed: {}\n", e).as_bytes());
            None
        }
    }
}

fn register_user(stream: &mut TcpStream, auth_manager: &Arc<Mutex<AuthManager>>) -> Option<user::User> {
    let _ = stream.write_all(b"Choose username: ");
    let username = read_line(stream)?;

    let _ = stream.write_all(b"Choose password: ");
    let password = read_line(stream)?;

    let mut auth = auth_manager.lock().unwrap();
    match auth.register(&username, &password) {
        Ok(user) => {
            let _ = stream.write_all(b"Registration successful! You are now logged in.\n");
            Some(user)
        }
        Err(e) => {
            let _ = stream.write_all(format!("Registration failed: {}\n", e).as_bytes());
            None
        }
    }
}

fn read_line(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = [0; 512];
    match stream.read(&mut buffer) {
        Ok(n) => Some(String::from_utf8_lossy(&buffer[..n]).trim().to_string()),
        Err(_) => None,
    }
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Server listening on 127.0.0.1:8080");

    let server = Arc::new(Server::new());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let server_clone = Arc::clone(&server);

                thread::spawn(move || {
                    handle_client(stream, server_clone);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}