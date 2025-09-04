use crate::types::{AuthLoginMessage, AuthResponse};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use tracing::{error, info, warn};

pub struct TestServer {
    port: u16,
    connections: Arc<RwLock<usize>>,
}

impl TestServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            connections: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("WebSocket test server listening on: {}", addr);

        // Print connection count periodically
        let connections_clone = Arc::clone(&self.connections);
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                let count = *connections_clone.read().await;
                info!("Active connections: {}", count);
            }
        });

        while let Ok((stream, addr)) = listener.accept().await {
            let connections = Arc::clone(&self.connections);
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, addr, connections).await {
                    error!("Error handling connection from {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        addr: std::net::SocketAddr,
        connections: Arc<RwLock<usize>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Increment connection count
        {
            let mut count = connections.write().await;
            *count += 1;
            info!("New connection from {} (total: {})", addr, *count);
        }

        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("WebSocket handshake failed for {}: {}", addr, e);
                // Decrement connection count on failure
                let mut count = connections.write().await;
                *count = count.saturating_sub(1);
                return Err(e.into());
            }
        };

        let (mut write, mut read) = ws_stream.split();

        // Echo messages back to client
        while let Some(msg) = read.next().await {
            match msg? {
                Message::Text(text) => {
                    // Check if this is an auth login message
                    if let Ok(auth_msg) = serde_json::from_str::<AuthLoginMessage>(&text) {
                        if auth_msg.namespace == "auth" && auth_msg.event == "login" {
                            // Respond with successful login
                            let response = AuthResponse {
                                status: "success".to_string(),
                                message: "Login successful".to_string(),
                            };
                            
                            let response_text = match serde_json::to_string(&response) {
                                Ok(json) => json,
                                Err(e) => {
                                    error!("Failed to serialize auth response: {}", e);
                                    continue;
                                }
                            };
                            
                            if let Err(e) = write.send(Message::Text(response_text)).await {
                                warn!("Failed to send auth response to {}: {}", addr, e);
                                break;
                            }
                        } else {
                            // Echo back the auth message for non-login events
                            if let Err(e) = write.send(Message::Text(text)).await {
                                warn!("Failed to echo auth message to {}: {}", addr, e);
                                break;
                            }
                        }
                    } else {
                        // Echo back regular messages
                        if let Err(e) = write.send(Message::Text(text)).await {
                            warn!("Failed to send message to {}: {}", addr, e);
                            break;
                        }
                    }
                }
                Message::Binary(data) => {
                    if let Err(e) = write.send(Message::Binary(data)).await {
                        warn!("Failed to send binary data to {}: {}", addr, e);
                        break;
                    }
                }
                Message::Close(_) => {
                    info!("Client {} initiated close", addr);
                    break;
                }
                Message::Ping(data) => {
                    if let Err(e) = write.send(Message::Pong(data)).await {
                        warn!("Failed to send pong to {}: {}", addr, e);
                        break;
                    }
                }
                _ => {}
            }
        }

        // Decrement connection count
        {
            let mut count = connections.write().await;
            *count = count.saturating_sub(1);
        }

        info!("Connection from {} closed", addr);
        Ok(())
    }
}