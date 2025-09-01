mod client;
mod user;
mod auth;
mod channel;
mod voice;

use crate::auth::AuthManager;
use crate::channel::{ChannelManager, ChannelType};
use crate::client::Client;
use crate::voice::VoiceChannelManager;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const MAX_CONNECTIONS: usize = 100;
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const BUFFER_SIZE: usize = 4096;

struct Server {
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    auth_manager: Arc<Mutex<AuthManager>>,
    channel_manager: Arc<Mutex<ChannelManager>>,
    voice_manager: Arc<Mutex<VoiceChannelManager>>,
    shutdown_tx: mpsc::Sender<()>,
    connection_count: Arc<Mutex<usize>>,
}

impl Server {
    fn new() -> (Self, mpsc::Receiver<()>) {
        let channel_manager = ChannelManager::new(); // Now loads channels automatically

        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        let server = Server {
            clients: Arc::new(Mutex::new(HashMap::new())),
            auth_manager: Arc::new(Mutex::new(AuthManager::new("users.json"))),
            channel_manager: Arc::new(Mutex::new(channel_manager)),
            voice_manager: Arc::new(Mutex::new(VoiceChannelManager::new())),
            shutdown_tx,
            connection_count: Arc::new(Mutex::new(0)),
        };

        (server, shutdown_rx)
    }

    fn can_accept_connection(&self) -> bool {
        match self.connection_count.lock() {
            Ok(count) => *count < MAX_CONNECTIONS,
            Err(_) => false,
        }
    }

    fn increment_connection_count(&self) -> bool {
        match self.connection_count.lock() {
            Ok(mut count) => {
                if *count < MAX_CONNECTIONS {
                    *count += 1;
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    fn decrement_connection_count(&self) {
        if let Ok(mut count) = self.connection_count.lock() {
            *count = count.saturating_sub(1);
        }
    }
}

type ServerResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn handle_client(mut stream: TcpStream, server: Arc<Server>) -> ServerResult<()> {
    // Set read timeout
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    
    let authenticated_user = match authenticate_client(&mut stream, &server.auth_manager) {
        Ok(user) => user,
        Err(e) => {
            let _ = stream.write_all(format!("Authentication failed: {}\n", e).as_bytes());
            return Ok(());
        }
    };

    println!("User {} authenticated successfully", authenticated_user.name);

    let client = match Client::new(stream.try_clone()?, authenticated_user) {
        Ok(client) => client,
        Err(e) => {
            let _ = stream.write_all(b"Failed to create client session\n");
            return Err(e.into());
        }
    };

    let client_id = client.id;

    // Show available channels
    if let Err(e) = show_channels(&mut stream, &server.channel_manager) {
        eprintln!("Failed to show channels to client: {}", e);
    }

    // Join general channel
    if let Ok(mut channel_manager) = server.channel_manager.lock() {
        channel_manager.join_channel("general", client.user.name.clone());
    }

    // Add client to server
    if let Ok(mut clients_guard) = server.clients.lock() {
        clients_guard.insert(client_id, client.try_clone()?);
    }

    // Broadcast join message
    broadcast_to_channel(
        &server.clients,
        &server.channel_manager,
        "general",
        &format!("*** {} joined the channel ***\n", client.user.name),
        Some(client_id),
    );

    // Send help message
    let help_msg = "\n=== Commands ===\n\
                   /channels - List all channels\n\
                   /join <channel> - Join a text channel\n\
                   /voice <channel> - Join a voice channel\n\
                   /leave - Leave current voice channel\n\
                   /create <name> text|voice - Create a new channel\n\
                   /users - List users in current channel\n\
                   /help - Show this help message\n\
                   /quit - Exit chat\n\
                   ================\n\n";
    let _ = stream.write_all(help_msg.as_bytes());

    let mut buffer = vec![0u8; BUFFER_SIZE];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Client disconnected
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[..n]).trim().to_string();

                if message == "/quit" {
                    let _ = stream.write_all(b"Goodbye!\n");
                    break;
                }

                if message.starts_with('/') {
                    if let Err(e) = handle_command(&mut stream, &server, &message, &client.user.name, client_id) {
                        eprintln!("Command handling error: {}", e);
                        let _ = stream.write_all(b"Command failed. Please try again.\n");
                    }
                } else {
                    // Handle regular message
                    let current_channel = get_client_current_channel(&server.clients, client_id);
                    
                    if let Some(channel) = current_channel {
                        let full_message = format!("[{}] {}: {}\n", channel, client.user.name, message);
                        broadcast_to_channel(&server.clients, &server.channel_manager,
                                             &channel, &full_message, Some(client_id));
                    }
                }
            }
            Err(e) => {
                match e.kind() {
                    std::io::ErrorKind::TimedOut => {
                        let _ = stream.write_all(b"Connection timed out due to inactivity.\n");
                    }
                    _ => {
                        eprintln!("Read error from client: {}", e);
                    }
                }
                break;
            }
        }
    }

    // Cleanup client
    cleanup_client(&server, client_id, &client.user.name);
    server.decrement_connection_count();
    
    println!("User {} disconnected", client.user.name);
    Ok(())
}

fn get_client_current_channel(clients: &Arc<Mutex<HashMap<Uuid, Client>>>, client_id: Uuid) -> Option<String> {
    clients.lock().ok()?
        .get(&client_id)?
        .current_channel.clone()
}

fn cleanup_client(server: &Arc<Server>, client_id: Uuid, username: &str) {
    // Get current channel before removing client
    let current_channel = get_client_current_channel(&server.clients, client_id);

    // Remove client from clients list
    if let Ok(mut clients_guard) = server.clients.lock() {
        clients_guard.remove(&client_id);
    }

    // Leave all channels
    if let Ok(mut channel_manager) = server.channel_manager.lock() {
        channel_manager.leave_all_channels(username);
    }

    // Leave voice channels
    if let Ok(mut voice_manager) = server.voice_manager.lock() {
        voice_manager.leave_voice_channel(username);
    }

    // Broadcast leave message
    if let Some(channel) = current_channel {
        broadcast_to_channel(&server.clients, &server.channel_manager,
                             &channel,
                             &format!("*** {} left the channel ***\n", username),
                             None);
    }
}

fn handle_command(stream: &mut TcpStream, server: &Arc<Server>, command: &str, username: &str, client_id: Uuid) -> ServerResult<()> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    match parts[0] {
        "/help" => {
            let help_msg = "\n=== Commands ===\n\
                           /channels - List all channels\n\
                           /join <channel> - Join a text channel\n\
                           /voice <channel> - Join a voice channel\n\
                           /leave - Leave current voice channel\n\
                           /create <name> text|voice - Create a new channel\n\
                           /users - List users in current channel\n\
                           /help - Show this help message\n\
                           /quit - Exit chat\n\
                           ================\n\n";
            stream.write_all(help_msg.as_bytes())?;
        }
        "/channels" => {
            show_channels(stream, &server.channel_manager)?;
        }
        "/join" => {
            handle_join_command(stream, server, &parts, username, client_id)?;
        }
        "/voice" => {
            handle_voice_command(stream, server, &parts, username)?;
        }
        "/leave" => {
            handle_leave_command(stream, server, username)?;
        }
        "/create" => {
            handle_create_command(stream, server, &parts)?;
        }
        "/users" => {
            handle_users_command(stream, server, client_id)?;
        }
        _ => {
            stream.write_all(b"Unknown command. Type /help for available commands.\n")?;
        }
    }
    Ok(())
}

fn handle_join_command(stream: &mut TcpStream, server: &Arc<Server>, parts: &[&str], username: &str, client_id: Uuid) -> ServerResult<()> {
    if parts.len() < 2 {
        stream.write_all(b"Usage: /join <channel_name>\n")?;
        return Ok(());
    }

    let channel_name = parts[1];
    let mut channel_manager = server.channel_manager.lock().map_err(|_| "Failed to acquire channel manager lock")?;

    if !channel_manager.channel_exists(channel_name) {
        stream.write_all(b"Channel does not exist\n")?;
        return Ok(());
    }

    // Get old channel
    let old_channel = get_client_current_channel(&server.clients, client_id);

    // Leave old channel
    if let Some(old) = &old_channel {
        channel_manager.leave_channel(old, username);
        broadcast_to_channel(&server.clients, &server.channel_manager,
                             old,
                             &format!("*** {} left the channel ***\n", username),
                             None);
    }

    // Join new channel
    channel_manager.join_channel(channel_name, username.to_string());

    // Update client's current channel
    if let Ok(mut clients) = server.clients.lock() {
        if let Some(client) = clients.get_mut(&client_id) {
            client.current_channel = Some(channel_name.to_string());
        }
    }

    stream.write_all(format!("Joined channel: {}\n", channel_name).as_bytes())?;
    broadcast_to_channel(&server.clients, &server.channel_manager,
                         channel_name,
                         &format!("*** {} joined the channel ***\n", username),
                         Some(client_id));

    Ok(())
}

fn handle_voice_command(stream: &mut TcpStream, server: &Arc<Server>, parts: &[&str], username: &str) -> ServerResult<()> {
    if parts.len() < 2 {
        stream.write_all(b"Usage: /voice <channel_name>\n")?;
        return Ok(());
    }

    let channel_name = parts[1];
    let channel_manager = server.channel_manager.lock().map_err(|_| "Failed to acquire channel manager lock")?;

    if let Some(channel) = channel_manager.get_channel(channel_name) {
        if channel.channel_type == ChannelType::Voice {
            let mut voice_manager = server.voice_manager.lock().map_err(|_| "Failed to acquire voice manager lock")?;
            voice_manager.join_voice_channel(username.to_string(), channel_name.to_string());
            stream.write_all(format!("Joined voice channel: {}\n", channel_name).as_bytes())?;
            stream.write_all(b"Note: Voice streaming not implemented. This is a placeholder.\n")?;
        } else {
            stream.write_all(b"That's not a voice channel\n")?;
        }
    } else {
        stream.write_all(b"Voice channel does not exist\n")?;
    }

    Ok(())
}

fn handle_leave_command(stream: &mut TcpStream, server: &Arc<Server>, username: &str) -> ServerResult<()> {
    let mut voice_manager = server.voice_manager.lock().map_err(|_| "Failed to acquire voice manager lock")?;
    if voice_manager.leave_voice_channel(username) {
        stream.write_all(b"Left voice channel\n")?;
    } else {
        stream.write_all(b"You're not in a voice channel\n")?;
    }
    Ok(())
}

fn handle_create_command(stream: &mut TcpStream, server: &Arc<Server>, parts: &[&str]) -> ServerResult<()> {
    if parts.len() < 3 {
        stream.write_all(b"Usage: /create <name> text|voice\n")?;
        return Ok(());
    }

    let channel_name = parts[1];
    let channel_type = match parts[2] {
        "text" => ChannelType::Text,
        "voice" => ChannelType::Voice,
        _ => {
            stream.write_all(b"Channel type must be 'text' or 'voice'\n")?;
            return Ok(());
        }
    };

    let mut channel_manager = server.channel_manager.lock().map_err(|_| "Failed to acquire channel manager lock")?;
    if channel_manager.create_channel(channel_name, channel_type) {
        stream.write_all(format!("Created {} channel: {}\n", parts[2], channel_name).as_bytes())?;
    } else {
        stream.write_all(b"Channel already exists\n")?;
    }

    Ok(())
}

fn handle_users_command(stream: &mut TcpStream, server: &Arc<Server>, client_id: Uuid) -> ServerResult<()> {
    let current_channel = get_client_current_channel(&server.clients, client_id);

    if let Some(channel) = current_channel {
        let channel_manager = server.channel_manager.lock().map_err(|_| "Failed to acquire channel manager lock")?;
        if let Some(ch) = channel_manager.get_channel(&channel) {
            let users = ch.users.join(", ");
            stream.write_all(format!("Users in {}: {}\n", channel, users).as_bytes())?;
        }
    } else {
        stream.write_all(b"You're not in any channel\n")?;
    }

    Ok(())
}

fn show_channels(stream: &mut TcpStream, channel_manager: &Arc<Mutex<ChannelManager>>) -> ServerResult<()> {
    let manager = channel_manager.lock().map_err(|_| "Failed to acquire channel manager lock")?;
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

    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn broadcast_to_channel(clients: &Arc<Mutex<HashMap<Uuid, Client>>>,
                        channel_manager: &Arc<Mutex<ChannelManager>>,
                        channel_name: &str,
                        message: &str,
                        exclude_client_id: Option<Uuid>) {
    // Get channel users
    let channel_users = if let Ok(manager) = channel_manager.lock() {
        manager.get_channel(channel_name)
            .map(|ch| ch.users.clone())
            .unwrap_or_default()
    } else {
        return;
    };

    // Get clients to broadcast to
    let clients_to_notify: Vec<Client> = if let Ok(clients_guard) = clients.lock() {
        clients_guard.values()
            .filter(|client| {
                channel_users.contains(&client.user.name) &&
                (exclude_client_id != Some(client.id))
            })
            .filter_map(|client| client.try_clone().ok())
            .collect()
    } else {
        return;
    };

    // Send messages (no locks held)
    for mut client in clients_to_notify {
        if client.stream.write_all(message.as_bytes()).is_err() {
            // Remove failed client
            if let Ok(mut clients_guard) = clients.lock() {
                clients_guard.remove(&client.id);
            }
        }
    }
}

fn authenticate_client(stream: &mut TcpStream, auth_manager: &Arc<Mutex<AuthManager>>) -> ServerResult<user::User> {
    stream.write_all(b"Welcome to the chat server!\n")?;
    stream.write_all(b"1. Login\n2. Register\nChoose option (1 or 2): ")?;

    let choice = read_line(stream)?;

    match choice.as_str() {
        "1" => login_user(stream, auth_manager),
        "2" => register_user(stream, auth_manager),
        _ => {
            stream.write_all(b"Invalid choice.\n")?;
            Err("Invalid authentication choice".into())
        }
    }
}

fn login_user(stream: &mut TcpStream, auth_manager: &Arc<Mutex<AuthManager>>) -> ServerResult<user::User> {
    stream.write_all(b"Username: ")?;
    let username = read_line(stream)?;

    stream.write_all(b"Password: ")?;
    let password = read_line(stream)?;

    let auth = auth_manager.lock().map_err(|_| "Failed to acquire auth manager lock")?;
    match auth.login(&username, &password) {
        Ok(user) => {
            stream.write_all(b"Login successful!\n")?;
            Ok(user)
        }
        Err(e) => {
            stream.write_all(format!("Login failed: {}\n", e).as_bytes())?;
            Err(e.into())
        }
    }
}

fn register_user(stream: &mut TcpStream, auth_manager: &Arc<Mutex<AuthManager>>) -> ServerResult<user::User> {
    stream.write_all(b"Choose username: ")?;
    let username = read_line(stream)?;

    stream.write_all(b"Choose password: ")?;
    let password = read_line(stream)?;

    let mut auth = auth_manager.lock().map_err(|_| "Failed to acquire auth manager lock")?;
    match auth.register(&username, &password) {
        Ok(user) => {
            stream.write_all(b"Registration successful! You are now logged in.\n")?;
            Ok(user)
        }
        Err(e) => {
            stream.write_all(format!("Registration failed: {}\n", e).as_bytes())?;
            Err(e.into())
        }
    }
}

fn read_line(stream: &mut TcpStream) -> ServerResult<String> {
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let n = stream.read(&mut buffer)?;
    if n == 0 {
        return Err("Connection closed".into());
    }
    Ok(String::from_utf8_lossy(&buffer[..n]).trim().to_string())
}

fn main() -> ServerResult<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Server listening on 127.0.0.1:8080");

    let (server, _shutdown_rx) = Server::new();
    let server = Arc::new(server);

    // Setup signal handling for graceful shutdown
    ctrlc::set_handler({
        let server = server.clone();
        move || {
            println!("\nShutting down server...");
            let _ = server.shutdown_tx.send(());
        }
    }).expect("Error setting Ctrl-C handler");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if !server.can_accept_connection() {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }

                if !server.increment_connection_count() {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }

                let server_clone = Arc::clone(&server);
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, server_clone) {
                        eprintln!("Client handling error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}