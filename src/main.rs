mod client;
mod user;
mod auth;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use crate::client::Client;
use crate::auth::AuthManager;

fn handle_client(mut stream: TcpStream, clients: Arc<Mutex<Vec<Client>>>, auth_manager: Arc<Mutex<AuthManager>>) {
    // Authentication phase
    let authenticated_user = match authenticate_client(&mut stream, &auth_manager) {
        Some(user) => user,
        None => {
            let _ = stream.write_all(b"Authentication failed. Disconnecting.\n");
            return;
        }
    };

    println!("User {} authenticated successfully", authenticated_user.name);

    let client = Client {
        stream: stream.try_clone().unwrap(),
        user: authenticated_user,
    };

    // Add client to the list
    {
        let mut clients_guard = clients.lock().unwrap();
        clients_guard.push(client);
    }

    // Broadcast that user joined
    broadcast_message(&clients, &format!("*** {} joined the chat ***\n",
                                         clients.lock().unwrap().last().unwrap().user.name), None);

    let mut buffer = [0; 512];
    let client_addr = stream.peer_addr().unwrap();

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Client disconnected
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[..n]);
                let username = {
                    let clients_guard = clients.lock().unwrap();
                    clients_guard.iter()
                        .find(|c| c.stream.peer_addr().unwrap() == client_addr)
                        .map(|c| c.user.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string())
                };

                let full_message = format!("{}: {}", username, message);
                broadcast_message(&clients, &full_message, Some(client_addr));
            }
            Err(_) => break,
        }
    }

    let username = {
        let mut clients_guard = clients.lock().unwrap();
        let username = clients_guard.iter()
            .find(|c| c.stream.peer_addr().unwrap() == client_addr)
            .map(|c| c.user.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        clients_guard.retain(|c| c.stream.peer_addr().unwrap() != client_addr);
        username
    };

    broadcast_message(&clients, &format!("*** {} left the chat ***\n", username), None);
    println!("User {} disconnected", username);
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

fn broadcast_message(clients: &Arc<Mutex<Vec<Client>>>, message: &str, exclude_addr: Option<std::net::SocketAddr>) {
    let mut clients_guard = clients.lock().unwrap();
    clients_guard.retain_mut(|client| {
        if let Some(addr) = exclude_addr {
            if client.stream.peer_addr().unwrap() == addr {
                return true; // Skip this client
            }
        }

        match client.stream.write_all(message.as_bytes()) {
            Ok(_) => true,
            Err(_) => false,
        }
    });
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Server listening on 127.0.0.1:8080");

    let clients = Arc::new(Mutex::new(Vec::new()));
    let auth_manager = Arc::new(Mutex::new(AuthManager::new("users.json")));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let clients_clone = Arc::clone(&clients);
                let auth_clone = Arc::clone(&auth_manager);

                thread::spawn(move || {
                    handle_client(stream, clients_clone, auth_clone);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}