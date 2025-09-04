use crate::stats::TestStats;
use crate::types::{DisconnectReason, TestMessage};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, WebSocketStream,
};
use tracing::{error, info, warn};
use url::Url;

pub struct LoadTester {
    url: String,
    num_connections: usize,
    message_interval_ms: u64,
    connection_timeout_ms: u64,
    message_timeout_ms: u64,
    reconnect_delay_ms: u64,
    stats: Arc<TestStats>,
    shutdown_signal: Arc<AtomicBool>,
}

impl LoadTester {
    pub fn new(
        url: String,
        num_connections: usize,
        message_interval_ms: u64,
        connection_timeout_ms: u64,
        message_timeout_ms: u64,
        reconnect_delay_ms: u64,
    ) -> Self {
        Self {
            url,
            num_connections,
            message_interval_ms,
            connection_timeout_ms,
            message_timeout_ms,
            reconnect_delay_ms,
            stats: Arc::new(TestStats::new()),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn validate_url(url: &str) -> Result<String, String> {
        let url = url.trim();
        
        // If URL doesn't start with ws:// or wss://, assume ws://
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            if url.starts_with("http://") {
                return Ok(url.replace("http://", "ws://"));
            } else if url.starts_with("https://") {
                return Ok(url.replace("https://", "wss://"));
            } else {
                return Ok(format!("ws://{}", url));
            }
        }
        
        // Validate the URL can be parsed
        match Url::parse(url) {
            Ok(_) => Ok(url.to_string()),
            Err(e) => Err(format!("Invalid URL: {}", e)),
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting endless WebSocket load test...");
        info!("Target URL: {}", self.url);
        info!("Number of connections: {}", self.num_connections);
        info!("Message interval: {} ms (targeting 1500 messages/min per connection)", self.message_interval_ms);
        info!("Expected total message rate: {:.0} messages/min", 
              self.num_connections as f64 * (60000.0 / self.message_interval_ms as f64));
        info!("Press Ctrl+C to stop and see final statistics");

        // Setup signal handler for graceful shutdown
        let shutdown_signal_clone = Arc::clone(&self.shutdown_signal);
        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("Received Ctrl+C, initiating graceful shutdown...");
                    shutdown_signal_clone.store(true, Ordering::Relaxed);
                }
                Err(err) => {
                    error!("Unable to listen for shutdown signal: {}", err);
                }
            }
        });

        let mut handles = Vec::new();

        // Spawn connection tasks with controlled rate
        for i in 0..self.num_connections {
            let url = self.url.clone();
            let stats = Arc::clone(&self.stats);
            let shutdown_signal = Arc::clone(&self.shutdown_signal);
            let shutdown_signal_break = Arc::clone(&self.shutdown_signal);
            let message_interval_ms = self.message_interval_ms;
            let connection_timeout_ms = self.connection_timeout_ms;
            let message_timeout_ms = self.message_timeout_ms;
            let reconnect_delay_ms = self.reconnect_delay_ms;

            let handle = tokio::spawn(async move {
                Self::maintain_connection(
                    i,
                    url,
                    stats,
                    shutdown_signal,
                    message_interval_ms,
                    connection_timeout_ms,
                    message_timeout_ms,
                    reconnect_delay_ms,
                ).await;
            });

            handles.push(handle);

            // Stagger connection attempts
            if i % 50 == 0 && i > 0 {
                info!("Created {} connections so far...", i);
                sleep(Duration::from_millis(2)).await;
            } else {
                sleep(Duration::from_millis(20)).await;
            }

            if shutdown_signal_break.load(Ordering::Relaxed) {
                break;
            }
        }

        // Start stats reporting
        let stats_clone = Arc::clone(&self.stats);
        let shutdown_clone = Arc::clone(&self.shutdown_signal);
        let stats_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                if shutdown_clone.load(Ordering::Relaxed) {
                    break;
                }
                stats_clone.print_stats();
            }
        });

        // Wait for shutdown signal
        while !self.shutdown_signal.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(100)).await;
        }

        info!("Shutdown signal received, waiting for connections to close...");
        
        // Wait for all connections to complete
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Connection task failed: {}", e);
            }
        }

        // Cancel stats reporting and print final stats
        stats_handle.abort();
        self.stats.print_final_stats();

        Ok(())
    }

    async fn maintain_connection(
        connection_id: usize,
        url: String,
        stats: Arc<TestStats>,
        shutdown_signal: Arc<AtomicBool>,
        message_interval_ms: u64,
        connection_timeout_ms: u64,
        message_timeout_ms: u64,
        reconnect_delay_ms: u64,
    ) {
        while !shutdown_signal.load(Ordering::Relaxed) {
            stats.connections_attempted.fetch_add(1, Ordering::Relaxed);

            let handshake_start = Instant::now();
            
            // Attempt connection with timeout and better error handling
            let connect_result = timeout(
                Duration::from_millis(connection_timeout_ms),
                connect_async(&url)
            ).await;

            let ws_stream = match connect_result {
                Ok(Ok((stream, response))) => {
                    let handshake_time = handshake_start.elapsed().as_millis() as u64;
                    stats.record_handshake_time(handshake_time);
                    
                    info!("Connection {} successful, response status: {:?}", connection_id, response.status());
                    stats.connections_successful.fetch_add(1, Ordering::Relaxed);
                    stats.connections_active.fetch_add(1, Ordering::Relaxed);
                    
                    // Extract socket addresses for port tracking
                    use tokio_tungstenite::MaybeTlsStream;
                    let (local_addr, remote_addr) = match stream.get_ref() {
                        MaybeTlsStream::Plain(tcp_stream) => (
                            tcp_stream.local_addr().ok(),
                            tcp_stream.peer_addr().ok(),
                        ),
                        #[cfg(feature = "native-tls")]
                        MaybeTlsStream::NativeTls(tls_stream) => {
                            let tcp_stream = tls_stream.get_ref();
                            (
                                tcp_stream.local_addr().ok(),
                                tcp_stream.peer_addr().ok(),
                            )
                        }
                        #[cfg(not(feature = "native-tls"))]
                        _ => (None, None),
                    };

                    if let (Some(local), Some(remote)) = (local_addr, remote_addr) {
                        stats.record_connection_ports(local.port(), remote.port());
                    }

                    stream
                }
                Ok(Err(e)) => {
                    error!("Connection {} failed with error: {}", connection_id, e);
                    stats.connections_failed.fetch_add(1, Ordering::Relaxed);
                    stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                    
                    // Wait before reconnecting
                    sleep(Duration::from_millis(reconnect_delay_ms)).await;
                    continue;
                }
                Err(_) => {
                    error!("Connection {} timed out after {} ms", connection_id, connection_timeout_ms);
                    stats.connections_failed.fetch_add(1, Ordering::Relaxed);
                    stats.timeout_errors.fetch_add(1, Ordering::Relaxed);
                    
                    // Wait before reconnecting
                    sleep(Duration::from_millis(reconnect_delay_ms)).await;
                    continue;
                }
            };

            let disconnect_reason = Self::handle_connected_session(
                connection_id,
                ws_stream,
                Arc::clone(&stats),
                Arc::clone(&shutdown_signal),
                message_interval_ms,
                message_timeout_ms,
            ).await;

            stats.connections_active.fetch_sub(1, Ordering::Relaxed);

            // Analyze disconnect reason
            match disconnect_reason {
                DisconnectReason::GracefulClose => {
                    info!("Connection {} closed gracefully", connection_id);
                }
                DisconnectReason::ServerKick => {
                    warn!("Connection {} was kicked by server!", connection_id);
                    stats.connections_server_kicked.fetch_add(1, Ordering::Relaxed);
                }
                DisconnectReason::UnexpectedClose => {
                    warn!("Connection {} closed unexpectedly", connection_id);
                    stats.connections_unexpected_close.fetch_add(1, Ordering::Relaxed);
                }
                DisconnectReason::Shutdown => {
                    info!("Connection {} closed due to shutdown", connection_id);
                    break;
                }
            }

            if !shutdown_signal.load(Ordering::Relaxed) {
                stats.connections_reconnected.fetch_add(1, Ordering::Relaxed);
                info!("Connection {} will reconnect in {} ms", connection_id, reconnect_delay_ms);
                sleep(Duration::from_millis(reconnect_delay_ms)).await;
            }
        }
    }

    async fn handle_connected_session(
        connection_id: usize,
        ws_stream: WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
        stats: Arc<TestStats>,
        shutdown_signal: Arc<AtomicBool>,
        message_interval_ms: u64,
        message_timeout_ms: u64,
    ) -> DisconnectReason {
        let (mut write, mut read) = ws_stream.split();
        let stats_clone = Arc::clone(&stats);
        let shutdown_clone = Arc::clone(&shutdown_signal);
        
        // Spawn message reading task
        let read_handle = tokio::spawn(async move {
            let mut last_server_message = Instant::now();
            
            while let Some(msg) = read.next().await {
                if shutdown_clone.load(Ordering::Relaxed) {
                    return DisconnectReason::Shutdown;
                }

                match msg {
                    Ok(message) => {
                        last_server_message = Instant::now();
                        stats_clone.record_message_received(&message);
                        
                        match &message {
                            Message::Text(text) => {
                                if let Ok(test_msg) = serde_json::from_str::<TestMessage>(text) {
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis() as u64;
                                    
                                    let latency = now.saturating_sub(test_msg.timestamp);
                                    stats_clone.messages_received.fetch_add(1, Ordering::Relaxed);
                                    stats_clone.record_latency(latency);
                                }
                            }
                            Message::Close(close_frame) => {
                                info!("Connection {} received close message: {:?}", connection_id, close_frame);
                                
                                // Analyze close frame to determine if server initiated
                                if let Some(frame) = close_frame {
                                    use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
                                    if frame.code == CloseCode::Normal || frame.code == CloseCode::Away {
                                        return DisconnectReason::GracefulClose;
                                    } else {
                                        // Server initiated close with error code
                                        return DisconnectReason::ServerKick;
                                    }
                                }
                                return DisconnectReason::GracefulClose;
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        warn!("Read error on connection {}: {}", connection_id, e);
                        stats_clone.receive_errors.fetch_add(1, Ordering::Relaxed);
                        
                        // Determine if this looks like a server kick
                        if last_server_message.elapsed() > Duration::from_secs(30) {
                            return DisconnectReason::ServerKick;
                        }
                        return DisconnectReason::UnexpectedClose;
                    }
                }
            }
            
            DisconnectReason::UnexpectedClose
        });

        // Small delay before starting to send messages
        sleep(Duration::from_millis(100)).await;

        // Send messages indefinitely until shutdown or error
        let mut interval = interval(Duration::from_millis(message_interval_ms));
        let mut msg_id = 0u64;

        loop {
            if shutdown_signal.load(Ordering::Relaxed) {
                break;
            }

            interval.tick().await;

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let test_msg = TestMessage {
                id: msg_id,
                timestamp,
                payload: format!("Test message {} from connection {}", msg_id, connection_id),
            };

            let msg_text = match serde_json::to_string(&test_msg) {
                Ok(text) => text,
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                    stats.protocol_errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            };

            let message = Message::Text(msg_text);
            stats.record_message_sent(&message);

            let send_result = timeout(
                Duration::from_millis(message_timeout_ms),
                write.send(message)
            ).await;

            match send_result {
                Ok(Ok(_)) => {
                    stats.messages_sent.fetch_add(1, Ordering::Relaxed);
                    msg_id = msg_id.wrapping_add(1);
                }
                Ok(Err(e)) => {
                    error!("Send error on connection {}: {}", connection_id, e);
                    stats.send_errors.fetch_add(1, Ordering::Relaxed);
                    stats.messages_failed.fetch_add(1, Ordering::Relaxed);
                    read_handle.abort();
                    return DisconnectReason::UnexpectedClose;
                }
                Err(_) => {
                    error!("Send timeout on connection {}", connection_id);
                    stats.timeout_errors.fetch_add(1, Ordering::Relaxed);
                    stats.messages_timeout.fetch_add(1, Ordering::Relaxed);
                    read_handle.abort();
                    return DisconnectReason::UnexpectedClose;
                }
            }
        }

        // Graceful shutdown
        let close_message = Message::Close(None);
        stats.record_message_sent(&close_message);
        let _ = write.send(close_message).await;
        
        match read_handle.await {
            Ok(reason) => reason,
            Err(_) => DisconnectReason::Shutdown,
        }
    }
}