use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMessage {
    pub id: u64,
    pub timestamp: u64,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLoginMessage {
    pub namespace: String,
    pub event: String,
    pub data: LoginData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginData {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug)]
pub enum DisconnectReason {
    GracefulClose,
    ServerKick,
    UnexpectedClose,
    Shutdown,
}