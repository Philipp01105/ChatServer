use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

fn handle_client(mut stream: TcpStream, clients: Arc<Mutex<Vec<TcpStream>>>) {
    let mut buffer = [0; 512];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                let message = &buffer[..n];

                let mut clients_guard = clients.lock().unwrap();
                for client in clients_guard.iter_mut() {
                    if client.peer_addr().unwrap() != stream.peer_addr().unwrap() {
                        let _ = client.write_all(message);
                    }
                }
            }
            Err(_) => break,
        }
    }

    let mut clients_guard = clients.lock().unwrap();
    clients_guard.retain(|c| c.peer_addr().unwrap() != stream.peer_addr().unwrap());
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Server listening on 127.0.0.1:8080");

    let clients = Arc::new(Mutex::new(Vec::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let clients_clone = Arc::clone(&clients);

                clients.lock().unwrap().push(stream.try_clone()?);

                thread::spawn(move || {
                    handle_client(stream, clients_clone);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}
