# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build and Run
```bash
cargo build              # Build the project
cargo run                # Run the chat server (listens on 127.0.0.1:8080)
cargo check              # Check for compilation errors without building
cargo clippy             # Run Rust linter
cargo fmt                # Format code
```

### Testing
Currently no automated tests are implemented. Test the server manually by connecting via telnet or TCP client to `127.0.0.1:8080`.

## Architecture Overview

ChatServer is a multi-threaded TCP chat server written in Rust. The server implements a Discord-like chat experience with channels, voice channels, and user authentication.

### Core Components

- **Server (`main.rs`)**: Central coordinator managing clients, authentication, channels, and voice
- **Client (`client.rs`)**: Represents connected users with TCP stream, user info, and current channel
- **Authentication (`auth.rs`)**: User registration/login with JSON file persistence (`users.json`)
- **Channel Management (`channel.rs`)**: Text/voice channel creation, joining, leaving, and user tracking
- **Voice Manager (`voice.rs`)**: Voice channel sessions with mute/deafen state (audio streaming not implemented)
- **User (`user.rs`)**: Simple user data structure with name and password

### Threading Model

Each client connection spawns a dedicated thread for handling messages and commands. The server uses `Arc<Mutex<>>` for shared state management across threads.

### Channel System

- **Text Channels**: Traditional chat channels for messaging
- **Voice Channels**: Special channels for voice communication (placeholder implementation)
- Default channels: "general" (text), "random" (text), "voice-lobby" (voice), "gaming" (voice)

### Command Protocol

Commands start with `/` and include:
- `/channels` - List all channels
- `/join <channel>` - Join text channel
- `/voice <channel>` - Join voice channel
- `/leave` - Leave current voice channel
- `/create <name> text|voice` - Create new channel
- `/users` - List users in current channel

### Data Persistence

User credentials are stored in `users.json` using serde JSON serialization. The file is created automatically on first registration.

## Code Patterns

- Extensive use of `Arc<Mutex<>>` for thread-safe shared state
- Error handling via `Result<T, String>` for user-facing operations
- TCP stream cloning for client management
- JSON serialization for data persistence
- Channel broadcasting with address exclusion to prevent echo