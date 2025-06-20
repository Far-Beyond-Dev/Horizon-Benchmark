//! Horizon Game Client - Fixed JSON serialization
//! 
//! A real game client for connecting to the Horizon game server.

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use url::Url;

// ============================================================================
// Game Types
// ============================================================================

type PlayerId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub position: Position,
    pub health: f32,
    pub mana: f32,
    pub level: u32,
    pub experience: u64,
}

#[derive(Debug, Clone)]
pub struct GameWorld {
    pub players: HashMap<PlayerId, Player>,
    pub my_player_id: Option<PlayerId>,
    pub chat_history: Vec<ChatMessage>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub from: String,
    pub message: String,
    pub channel: String,
    pub timestamp: Instant,
}

// ============================================================================
// Network Messages - FIXED SERIALIZATION
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum NetworkMessage {
    Join { 
        name: String 
    },
    Move { 
        position: Position 
    },
    Leave,
    Data { 
        event_type: String, 
        data: serde_json::Value 
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum NetworkResponse {
    JoinSuccess { player_id: PlayerId },
    JoinFailed { reason: String },
    MoveSuccess,
    MoveFailed { reason: String },
    Data { event_type: String, data: serde_json::Value },
    Error { message: String },
}

// ============================================================================
// Game Client
// ============================================================================

pub struct HorizonGameClient {
    pub player_name: String,
    pub server_url: String,
    pub world: Arc<RwLock<GameWorld>>,
    pub websocket_tx: Arc<Mutex<Option<mpsc::UnboundedSender<NetworkMessage>>>>,
    pub shutdown_tx: broadcast::Sender<()>,
}

// Define a custom error type
#[derive(Debug)]
pub struct ClientError {
    pub message: String,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ClientError {}

impl From<Box<dyn std::error::Error + Send + Sync>> for ClientError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        ClientError {
            message: err.to_string(),
        }
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for ClientError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        ClientError {
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        ClientError {
            message: err.to_string(),
        }
    }
}

impl From<url::ParseError> for ClientError {
    fn from(err: url::ParseError) -> Self {
        ClientError {
            message: err.to_string(),
        }
    }
}

impl From<mpsc::error::SendError<NetworkMessage>> for ClientError {
    fn from(err: mpsc::error::SendError<NetworkMessage>) -> Self {
        ClientError {
            message: format!("Send error: {}", err),
        }
    }
}

impl HorizonGameClient {
    pub fn new(player_name: String, server_url: String) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        
        Self {
            player_name,
            server_url,
            world: Arc::new(RwLock::new(GameWorld {
                players: HashMap::new(),
                my_player_id: None,
                chat_history: Vec::new(),
            })),
            websocket_tx: Arc::new(Mutex::new(None)),
            shutdown_tx,
        }
    }

    pub async fn connect_and_play(&self) -> Result<(), ClientError> {
        println!("🎮 Horizon Game Client");
        println!("Player: {} | Server: {}", self.player_name, self.server_url);
        println!("Date: 2025-06-20 22:01:50 UTC | User: tristanpoland");
        println!("===============================================");
        
        let url = Url::parse(&self.server_url)?;
        info!("🔗 Connecting to Horizon server...");
        
        let (ws_stream, _) = connect_async(url).await?;
        println!("✅ Connected to Horizon!");
        
        let (ws_sender, mut ws_receiver) = ws_stream.split();
        let ws_sender = Arc::new(Mutex::new(ws_sender));
        
        // Create message channel
        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<NetworkMessage>();
        
        // Store sender for game commands
        {
            let mut tx_guard = self.websocket_tx.lock().await;
            *tx_guard = Some(msg_tx.clone());
        }

        // Send join message
        let join_msg = NetworkMessage::Join {
            name: self.player_name.clone(),
        };
        msg_tx.send(join_msg)?;

        // Setup tasks
        let world = self.world.clone();
        let shutdown_tx = self.shutdown_tx.clone();
        
        // Outgoing messages task - FIXED
        let outgoing_task = {
            let ws_sender = ws_sender.clone();
            
            async move {
                while let Some(message) = msg_rx.recv().await {
                    // Debug: Show what we're sending
                    match serde_json::to_string(&message) {
                        Ok(msg_text) => {
                            debug!("📤 Sending: {}", msg_text);
                            
                            let mut sender = ws_sender.lock().await;
                            if let Err(e) = sender.send(Message::Text(msg_text)).await {
                                error!("Failed to send message: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize message {:?}: {}", message, e);
                            break;
                        }
                    }
                }
                Ok::<(), ClientError>(())
            }
        };

        // Incoming messages task
        let incoming_task = {
            let world = world.clone();
            
            async move {
                while let Some(msg) = ws_receiver.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            debug!("📥 Received: {}", text);
                            if let Err(e) = Self::handle_server_message(&text, &world).await {
                                error!("Error handling server message: {}", e);
                            }
                        }
                        Ok(Message::Close(_)) => {
                            println!("🔌 Server disconnected");
                            break;
                        }
                        Ok(Message::Ping(data)) => {
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Pong(data)).await;
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                Ok::<(), ClientError>(())
            }
        };

        // Game UI task
        let ui_task = {
            let world = world.clone();
            let websocket_tx = self.websocket_tx.clone();
            let shutdown_tx = shutdown_tx.clone();
            
            async move {
                Self::run_game_ui(world, websocket_tx, shutdown_tx).await;
            }
        };

        // Run all tasks
        tokio::select! {
            result = outgoing_task => {
                if let Err(e) = result {
                    error!("Outgoing task error: {}", e);
                }
            }
            result = incoming_task => {
                if let Err(e) = result {
                    error!("Incoming task error: {}", e);
                }
            }
            _ = ui_task => {
                println!("Game UI ended");
            }
        }

        println!("👋 Disconnected from Horizon. Thanks for playing!");
        Ok(())
    }

    async fn handle_server_message(
        text: &str, 
        world: &Arc<RwLock<GameWorld>>
    ) -> Result<(), ClientError> {
        
        match serde_json::from_str::<NetworkResponse>(text) {
            Ok(response) => {
                match response {
                    NetworkResponse::JoinSuccess { player_id } => {
                        let mut world_guard = world.write().await;
                        world_guard.my_player_id = Some(player_id);
                        println!("🎉 Successfully joined Horizon as Player {}!", player_id);
                        println!("Type 'help' for available commands");
                    }
                    
                    NetworkResponse::JoinFailed { reason } => {
                        println!("❌ Failed to join: {}", reason);
                    }
                    
                    NetworkResponse::MoveSuccess => {
                        println!("🚶 Movement successful");
                    }
                    
                    NetworkResponse::MoveFailed { reason } => {
                        println!("❌ Movement failed: {}", reason);
                    }
                    
                    NetworkResponse::Data { event_type, data } => {
                        Self::handle_game_event(&event_type, &data, world).await?;
                    }
                    
                    NetworkResponse::Error { message } => {
                        println!("⚠️ Server error: {}", message);
                    }
                }
            }
            Err(e) => {
                debug!("Failed to parse server message: {} - Raw: {}", e, text);
            }
        }
        
        Ok(())
    }

    async fn handle_game_event(
        event_type: &str,
        data: &serde_json::Value,
        _world: &Arc<RwLock<GameWorld>>
    ) -> Result<(), ClientError> {
        
        match event_type {
            "welcome" => {
                if let Some(content) = data.get("content") {
                    if let Some(message) = content.get("message") {
                        println!("💬 Welcome: {}", message.as_str().unwrap_or(""));
                    }
                }
            }
            
            "stats_response" => {
                if let Some(content) = data.get("content") {
                    println!("📊 Your Stats:");
                    if let Some(basic) = content.get("basic") {
                        println!("  Messages sent: {}", basic.get("messages_sent").unwrap_or(&serde_json::Value::Null));
                        println!("  Events: {}", basic.get("total_events").unwrap_or(&serde_json::Value::Null));
                        println!("  Distance moved: {:.1}", basic.get("total_distance_moved").unwrap_or(&serde_json::Value::Null));
                    }
                }
            }
            
            "help_response" => {
                if let Some(content) = data.get("content") {
                    println!("📖 Available Commands:");
                    if let Some(commands) = content.get("commands").and_then(|c| c.as_array()) {
                        for cmd in commands {
                            println!("  {}", cmd.as_str().unwrap_or(""));
                        }
                    }
                }
            }
            
            "crafting_response" => {
                if let Some(content) = data.get("content") {
                    println!("🔨 Crafting: Recipe {} - {}", 
                             content.get("recipe_id").unwrap_or(&serde_json::Value::Null),
                             content.get("status").unwrap_or(&serde_json::Value::Null));
                    if let Some(tip) = content.get("assistance_tip") {
                        println!("💡 Tip: {}", tip.as_str().unwrap_or(""));
                    }
                }
            }
            
            "interaction_response" => {
                if let Some(content) = data.get("content") {
                    println!("👥 Interaction: {} with Player {}", 
                             content.get("interaction_type").unwrap_or(&serde_json::Value::Null),
                             content.get("target_player").unwrap_or(&serde_json::Value::Null));
                }
            }
            
            "greeting_response" => {
                if let Some(content) = data.get("content") {
                    if let Some(message) = content.get("message") {
                        println!("👋 {}", message.as_str().unwrap_or(""));
                    }
                }
            }
            
            "ack" => {
                // Acknowledgment from server - just debug log it
                debug!("✅ Server acknowledged: {:?}", data);
            }
            
            _ => {
                debug!("Unknown game event: {} - Data: {:?}", event_type, data);
            }
        }
        
        Ok(())
    }

    async fn run_game_ui(
        world: Arc<RwLock<GameWorld>>,
        websocket_tx: Arc<Mutex<Option<mpsc::UnboundedSender<NetworkMessage>>>>,
        shutdown_tx: broadcast::Sender<()>,
    ) {
        println!("\n🎮 Game Commands:");
        println!("  chat <message>       - Send chat message");
        println!("  move <x> <y> <z>     - Move to coordinates");
        println!("  attack <ability>     - Attack with ability");
        println!("  craft <recipe> <qty> - Craft items");
        println!("  stats                - View your stats");
        println!("  players              - List online players");
        println!("  help                 - Get help from server");
        println!("  quit                 - Leave the game");
        
        loop {
            print!("\ntristanpoland@horizon> ");
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }
            
            let input = input.trim();
            if input.is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = input.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            
            match parts[0] {
                "quit" | "exit" => {
                    Self::send_game_message(NetworkMessage::Leave, &websocket_tx).await;
                    let _ = shutdown_tx.send(());
                    break;
                }
                
                "chat" => {
                    if parts.len() > 1 {
                        let message = parts[1..].join(" ");
                        let event_data = serde_json::json!({
                            "message": message,
                            "channel": "global"
                        });
                        
                        let msg = NetworkMessage::Data {
                            event_type: "chat_message".to_string(),
                            data: event_data,
                        };
                        
                        Self::send_game_message(msg, &websocket_tx).await;
                        println!("💬 You: {}", message);
                    } else {
                        println!("Usage: chat <message>");
                    }
                }
                
                "move" => {
                    if parts.len() == 4 {
                        if let (Ok(x), Ok(y), Ok(z)) = (
                            parts[1].parse::<f64>(),
                            parts[2].parse::<f64>(),
                            parts[3].parse::<f64>(),
                        ) {
                            let position = Position::new(x, y, z);
                            
                            // Send the core Move message for server handling
                            let move_msg = NetworkMessage::Move { position: position.clone() };
                            Self::send_game_message(move_msg, &websocket_tx).await;
                            
                            // Also send as plugin event for enhanced features
                            let event_data = serde_json::json!({
                                "target_x": x,
                                "target_y": y,
                                "target_z": z,
                                "movement_type": "Walk"
                            });
                            
                            let event_msg = NetworkMessage::Data {
                                event_type: "move_command".to_string(),
                                data: event_data,
                            };
                            
                            Self::send_game_message(event_msg, &websocket_tx).await;
                            println!("🚶 Moving to ({:.1}, {:.1}, {:.1})", x, y, z);
                        } else {
                            println!("Usage: move <x> <y> <z> (numbers only)");
                        }
                    } else {
                        println!("Usage: move <x> <y> <z>");
                    }
                }
                
                "attack" => {
                    if parts.len() >= 2 {
                        let ability_id: u32 = parts[1].parse().unwrap_or(1);
                        let event_data = serde_json::json!({
                            "action_type": "Attack",
                            "ability_id": ability_id
                        });
                        
                        let msg = NetworkMessage::Data {
                            event_type: "combat_action".to_string(),
                            data: event_data,
                        };
                        
                        Self::send_game_message(msg, &websocket_tx).await;
                        println!("⚔️ Attacking with ability {}!", ability_id);
                    } else {
                        println!("Usage: attack <ability_id>");
                    }
                }
                
                "craft" => {
                    if parts.len() >= 3 {
                        if let (Ok(recipe_id), Ok(quantity)) = (
                            parts[1].parse::<u32>(),
                            parts[2].parse::<u32>(),
                        ) {
                            let event_data = serde_json::json!({
                                "recipe_id": recipe_id,
                                "quantity": quantity,
                                "ingredient_sources": []
                            });
                            
                            let msg = NetworkMessage::Data {
                                event_type: "crafting_request".to_string(),
                                data: event_data,
                            };
                            
                            Self::send_game_message(msg, &websocket_tx).await;
                            println!("🔨 Crafting {} x Recipe {}...", quantity, recipe_id);
                        } else {
                            println!("Usage: craft <recipe_id> <quantity> (numbers only)");
                        }
                    } else {
                        println!("Usage: craft <recipe_id> <quantity>");
                    }
                }
                
                "stats" => {
                    let msg = NetworkMessage::Data {
                        event_type: "chat_message".to_string(),
                        data: serde_json::json!({
                            "message": "!stats",
                            "channel": "global"
                        }),
                    };
                    
                    Self::send_game_message(msg, &websocket_tx).await;
                    println!("📊 Requesting stats...");
                }
                
                "help" => {
                    let msg = NetworkMessage::Data {
                        event_type: "chat_message".to_string(),
                        data: serde_json::json!({
                            "message": "!help",
                            "channel": "global"
                        }),
                    };
                    
                    Self::send_game_message(msg, &websocket_tx).await;
                    println!("📖 Requesting help...");
                }
                
                "players" => {
                    let world_guard = world.read().await;
                    println!("👥 Online Players:");
                    if world_guard.players.is_empty() {
                        println!("  No other players visible");
                    } else {
                        for player in world_guard.players.values() {
                            println!("  {} (ID: {}) at ({:.1}, {:.1}, {:.1})", 
                                   player.name, player.id, 
                                   player.position.x, player.position.y, player.position.z);
                        }
                    }
                }
                
                "debug" => {
                    // Secret debug command
                    println!("🔧 Debug info:");
                    let tx_guard = websocket_tx.lock().await;
                    println!("  Connected: {}", tx_guard.is_some());
                    let world_guard = world.read().await;
                    println!("  Player ID: {:?}", world_guard.my_player_id);
                    println!("  Players in world: {}", world_guard.players.len());
                }
                
                _ => {
                    println!("Unknown command: '{}'. Type 'help' for available commands.", parts[0]);
                }
            }
        }
        
        println!("👋 Leaving Horizon...");
    }

    async fn send_game_message(
        message: NetworkMessage,
        websocket_tx: &Arc<Mutex<Option<mpsc::UnboundedSender<NetworkMessage>>>>
    ) {
        let tx_guard = websocket_tx.lock().await;
        if let Some(sender) = tx_guard.as_ref() {
            if let Err(e) = sender.send(message.clone()) {
                println!("Failed to send game message: {}", e);
            } else {
                println!("📤 Sent game message: {:?}", message);
            }
        } else {
            println!("⚠️ Not connected to server");
        }
    }
}

// ============================================================================
// Main Function
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let args: Vec<String> = std::env::args().collect();
    
    // Get player name
    let player_name = if args.len() > 1 {
        args[1].clone()
    } else {
        print!("Enter your player name: ");
        io::stdout().flush().unwrap();
        let mut name = String::new();
        io::stdin().read_line(&mut name).unwrap();
        name.trim().to_string()
    };
    
    if player_name.is_empty() {
        println!("Player name cannot be empty!");
        return Ok(());
    }
    
    // Get server URL
    let server_url = if args.len() > 2 {
        args[2].clone()
    } else {
        "ws://127.0.0.1:8080".to_string()
    };
    
    // Create and start game client
    let client = HorizonGameClient::new(player_name, server_url);
    
    if let Err(e) = client.connect_and_play().await {
        eprintln!("❌ Game client error: {}", e);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_serialization() {
        let pos = Position::new(1.5, 2.5, 3.5);
        let json = serde_json::to_string(&pos).unwrap();
        println!("Position JSON: {}", json);
        
        let parsed: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.x, 1.5);
        assert_eq!(parsed.y, 2.5);
        assert_eq!(parsed.z, 3.5);
    }

    #[test]
    fn test_move_message_serialization() {
        let move_msg = NetworkMessage::Move {
            position: Position::new(10.0, 5.0, 0.0)
        };
        
        let json = serde_json::to_string(&move_msg).unwrap();
        println!("Move message JSON: {}", json);
        
        // Should serialize to: {"type":"Move","position":{"x":10.0,"y":5.0,"z":0.0}}
        assert!(json.contains("\"type\":\"Move\""));
        assert!(json.contains("\"position\""));
        assert!(json.contains("\"x\":10.0"));
    }

    #[test]
    fn test_data_message_serialization() {
        let data_msg = NetworkMessage::Data {
            event_type: "test".to_string(),
            data: serde_json::json!({"test": "value"})
        };
        
        let json = serde_json::to_string(&data_msg).unwrap();
        println!("Data message JSON: {}", json);
        
        // Should have both event_type and data fields
        assert!(json.contains("\"type\":\"Data\""));
        assert!(json.contains("\"event_type\":\"test\""));
        assert!(json.contains("\"data\""));
    }
}