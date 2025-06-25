use clap::{Arg, Command};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite::protocol::Message, WebSocketStream,
};
use tracing::{error, info, warn};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestMessage {
    id: u64,
    timestamp: u64,
    payload: String,
}

#[derive(Debug)]
struct TestStats {
    connections_attempted: AtomicUsize,
    connections_successful: AtomicUsize,
    connections_failed: AtomicUsize,
    connections_active: AtomicUsize,
    connections_server_kicked: AtomicUsize,
    connections_unexpected_close: AtomicUsize,
    connections_reconnected: AtomicUsize,
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    total_latency_ms: AtomicU64,
    max_latency_ms: AtomicU64,
    min_latency_ms: AtomicU64,
    start_time: Instant,
}

impl Clone for TestStats {
    fn clone(&self) -> Self {
        Self {
            connections_attempted: AtomicUsize::new(self.connections_attempted.load(Ordering::Relaxed)),
            connections_successful: AtomicUsize::new(self.connections_successful.load(Ordering::Relaxed)),
            connections_failed: AtomicUsize::new(self.connections_failed.load(Ordering::Relaxed)),
            connections_active: AtomicUsize::new(self.connections_active.load(Ordering::Relaxed)),
            connections_server_kicked: AtomicUsize::new(self.connections_server_kicked.load(Ordering::Relaxed)),
            connections_unexpected_close: AtomicUsize::new(self.connections_unexpected_close.load(Ordering::Relaxed)),
            connections_reconnected: AtomicUsize::new(self.connections_reconnected.load(Ordering::Relaxed)),
            messages_sent: AtomicU64::new(self.messages_sent.load(Ordering::Relaxed)),
            messages_received: AtomicU64::new(self.messages_received.load(Ordering::Relaxed)),
            total_latency_ms: AtomicU64::new(self.total_latency_ms.load(Ordering::Relaxed)),
            max_latency_ms: AtomicU64::new(self.max_latency_ms.load(Ordering::Relaxed)),
            min_latency_ms: AtomicU64::new(self.min_latency_ms.load(Ordering::Relaxed)),
            start_time: self.start_time,
        }
    }
}

impl TestStats {
    fn new() -> Self {
        Self {
            connections_attempted: AtomicUsize::new(0),
            connections_successful: AtomicUsize::new(0),
            connections_failed: AtomicUsize::new(0),
            connections_active: AtomicUsize::new(0),
            connections_server_kicked: AtomicUsize::new(0),
            connections_unexpected_close: AtomicUsize::new(0),
            connections_reconnected: AtomicUsize::new(0),
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            max_latency_ms: AtomicU64::new(0),
            min_latency_ms: AtomicU64::new(u64::MAX),
            start_time: Instant::now(),
        }
    }

    fn record_latency(&self, latency_ms: u64) {
        self.total_latency_ms.fetch_add(latency_ms, Ordering::Relaxed);
        
        // Update max latency using simple compare and swap
        loop {
            let current_max = self.max_latency_ms.load(Ordering::Relaxed);
            if latency_ms <= current_max {
                break;
            }
            if self.max_latency_ms.compare_exchange(
                current_max,
                latency_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }
        }

        // Update min latency using simple compare and swap
        loop {
            let current_min = self.min_latency_ms.load(Ordering::Relaxed);
            if latency_ms >= current_min {
                break;
            }
            if self.min_latency_ms.compare_exchange(
                current_min,
                latency_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }
        }
    }

    fn print_stats(&self) {
        let attempted = self.connections_attempted.load(Ordering::Relaxed);
        let successful = self.connections_successful.load(Ordering::Relaxed);
        let failed = self.connections_failed.load(Ordering::Relaxed);
        let active = self.connections_active.load(Ordering::Relaxed);
        let server_kicked = self.connections_server_kicked.load(Ordering::Relaxed);
        let unexpected_close = self.connections_unexpected_close.load(Ordering::Relaxed);
        let reconnected = self.connections_reconnected.load(Ordering::Relaxed);
        let sent = self.messages_sent.load(Ordering::Relaxed);
        let received = self.messages_received.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ms.load(Ordering::Relaxed);
        let max_latency = self.max_latency_ms.load(Ordering::Relaxed);
        let min_latency = self.min_latency_ms.load(Ordering::Relaxed);

        let avg_latency = if received > 0 {
            total_latency / received
        } else {
            0
        };

        let min_display = if min_latency == u64::MAX { 0 } else { min_latency };
        let uptime = self.start_time.elapsed();

        info!("=== WebSocket Test Statistics ===");
        info!("Test uptime: {:.2} hours ({:.0} seconds)", uptime.as_secs_f64() / 3600.0, uptime.as_secs_f64());
        info!("Connections attempted: {}", attempted);
        info!("Connections successful: {}", successful);
        info!("Connections active: {}", active);
        info!("Connections failed: {}", failed);
        info!("Connections server-kicked: {}", server_kicked);
        info!("Connections unexpected close: {}", unexpected_close);
        info!("Connections reconnected: {}", reconnected);
        info!("Success rate: {:.2}%", if attempted > 0 { (successful as f64 / attempted as f64) * 100.0 } else { 0.0 });
        info!("Messages sent: {}", sent);
        info!("Messages received: {}", received);
        info!("Message success rate: {:.2}%", if sent > 0 { (received as f64 / sent as f64) * 100.0 } else { 0.0 });
        info!("Messages/sec sent: {:.2}", if uptime.as_secs() > 0 { sent as f64 / uptime.as_secs_f64() } else { 0.0 });
        info!("Messages/sec received: {:.2}", if uptime.as_secs() > 0 { received as f64 / uptime.as_secs_f64() } else { 0.0 });
        info!("Average latency: {} ms", avg_latency);
        info!("Min latency: {} ms", min_display);
        info!("Max latency: {} ms", max_latency);
        
        // Calculate disconnect rates
        let total_disconnects = server_kicked + unexpected_close;
        if total_disconnects > 0 {
            warn!("=== DISCONNECT ANALYSIS ===");
            warn!("Total disconnects: {}", total_disconnects);
            warn!("Server kicks: {} ({:.2}%)", server_kicked, (server_kicked as f64 / total_disconnects as f64) * 100.0);
            warn!("Unexpected closes: {} ({:.2}%)", unexpected_close, (unexpected_close as f64 / total_disconnects as f64) * 100.0);
            warn!("Disconnect rate: {:.4}/hour", total_disconnects as f64 / (uptime.as_secs_f64() / 3600.0));
        }
    }

    fn print_final_stats(&self) {
        info!("\n{}", "=".repeat(50));
        info!("          FINAL TEST RESULTS");
        info!("{}", "=".repeat(50));
        self.print_stats();
        info!("{}", "=".repeat(50));
    }
}

struct LoadTester {
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
    fn new(
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

    fn validate_url(url: &str) -> Result<String, String> {
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

    async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting endless WebSocket load test...");
        info!("Target URL: {}", self.url);
        info!("Number of connections: {}", self.num_connections);
        info!("Message interval: {} ms", self.message_interval_ms);
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
                sleep(Duration::from_millis(100)).await;
            } else {
                sleep(Duration::from_millis(20)).await;
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

            // Attempt connection with timeout and better error handling
            let connect_result = timeout(
                Duration::from_millis(connection_timeout_ms),
                connect_async(&url)
            ).await;

            let ws_stream = match connect_result {
                Ok(Ok((stream, response))) => {
                    info!("Connection {} successful, response status: {:?}", connection_id, response.status());
                    stats.connections_successful.fetch_add(1, Ordering::Relaxed);
                    stats.connections_active.fetch_add(1, Ordering::Relaxed);
                    stream
                }
                Ok(Err(e)) => {
                    error!("Connection {} failed with error: {}", connection_id, e);
                    stats.connections_failed.fetch_add(1, Ordering::Relaxed);
                    
                    // Wait before reconnecting
                    sleep(Duration::from_millis(reconnect_delay_ms)).await;
                    continue;
                }
                Err(_) => {
                    error!("Connection {} timed out after {} ms", connection_id, connection_timeout_ms);
                    stats.connections_failed.fetch_add(1, Ordering::Relaxed);
                    
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
                    Ok(Message::Text(text)) => {
                        last_server_message = Instant::now();
                        if let Ok(test_msg) = serde_json::from_str::<TestMessage>(&text) {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;
                            
                            let latency = now.saturating_sub(test_msg.timestamp);
                            stats_clone.messages_received.fetch_add(1, Ordering::Relaxed);
                            stats_clone.record_latency(latency);
                        }
                    }
                    Ok(Message::Close(close_frame)) => {
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
                    Ok(Message::Pong(_)) => {
                        last_server_message = Instant::now();
                    }
                    Err(e) => {
                        warn!("Read error on connection {}: {}", connection_id, e);
                        
                        // Determine if this looks like a server kick
                        if last_server_message.elapsed() > Duration::from_secs(30) {
                            return DisconnectReason::ServerKick;
                        }
                        return DisconnectReason::UnexpectedClose;
                    }
                    _ => {
                        last_server_message = Instant::now();
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
                    continue;
                }
            };

            let send_result = timeout(
                Duration::from_millis(message_timeout_ms),
                write.send(Message::Text(msg_text))
            ).await;

            match send_result {
                Ok(Ok(_)) => {
                    stats.messages_sent.fetch_add(1, Ordering::Relaxed);
                    msg_id = msg_id.wrapping_add(1);
                }
                Ok(Err(e)) => {
                    error!("Send error on connection {}: {}", connection_id, e);
                    read_handle.abort();
                    return DisconnectReason::UnexpectedClose;
                }
                Err(_) => {
                    error!("Send timeout on connection {}", connection_id);
                    read_handle.abort();
                    return DisconnectReason::UnexpectedClose;
                }
            }
        }

        // Graceful shutdown
        let _ = write.send(Message::Close(None)).await;
        
        match read_handle.await {
            Ok(reason) => reason,
            Err(_) => DisconnectReason::Shutdown,
        }
    }
}

#[derive(Debug)]
enum DisconnectReason {
    GracefulClose,
    ServerKick,
    UnexpectedClose,
    Shutdown,
}

// Simple WebSocket echo server for testing
struct TestServer {
    port: u16,
    connections: Arc<RwLock<usize>>,
}

impl TestServer {
    fn new(port: u16) -> Self {
        Self {
            port,
            connections: Arc::new(RwLock::new(0)),
        }
    }

    async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
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
                    // Echo back the same message
                    if let Err(e) = write.send(Message::Text(text)).await {
                        warn!("Failed to send message to {}: {}", addr, e);
                        break;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let matches = Command::new("WebSocket Load Tester")
        .version("2.0")
        .author("Your Name")
        .about("High-performance endless WebSocket load testing tool")
        .subcommand(
            Command::new("test")
                .about("Run endless load test against a WebSocket server")
                .arg(
                    Arg::new("url")
                        .short('u')
                        .long("url")
                        .value_name("URL")
                        .help("WebSocket server URL")
                        .required(true),
                )
                .arg(
                    Arg::new("connections")
                        .short('c')
                        .long("connections")
                        .value_name("NUM")
                        .help("Number of concurrent connections to maintain")
                        .default_value("100"),
                )
                .arg(
                    Arg::new("interval")
                        .short('i')
                        .long("interval")
                        .value_name("MS")
                        .help("Interval between messages in milliseconds")
                        .default_value("1000"),
                )
                .arg(
                    Arg::new("connection-timeout")
                        .long("connection-timeout")
                        .value_name("MS")
                        .help("Connection timeout in milliseconds")
                        .default_value("5000"),
                )
                .arg(
                    Arg::new("message-timeout")
                        .long("message-timeout")
                        .value_name("MS")
                        .help("Message timeout in milliseconds")
                        .default_value("5000"),
                )
                .arg(
                    Arg::new("reconnect-delay")
                        .long("reconnect-delay")
                        .value_name("MS")
                        .help("Delay before reconnecting after disconnect in milliseconds")
                        .default_value("1000"),
                ),
        )
        .subcommand(
            Command::new("server")
                .about("Run a test WebSocket server")
                .arg(
                    Arg::new("port")
                        .short('p')
                        .long("port")
                        .value_name("PORT")
                        .help("Port to listen on")
                        .default_value("8080"),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("test", test_matches)) => {
            let raw_url = test_matches.get_one::<String>("url").unwrap();
            let url = match LoadTester::validate_url(raw_url) {
                Ok(validated_url) => {
                    info!("Using WebSocket URL: {}", validated_url);
                    validated_url
                },
                Err(e) => {
                    error!("URL validation failed: {}", e);
                    return Err(e.into());
                }
            };
            
            let connections = test_matches
                .get_one::<String>("connections")
                .unwrap()
                .parse::<usize>()?;
            let interval = test_matches
                .get_one::<String>("interval")
                .unwrap()
                .parse::<u64>()?;
            let connection_timeout = test_matches
                .get_one::<String>("connection-timeout")
                .unwrap()
                .parse::<u64>()?;
            let message_timeout = test_matches
                .get_one::<String>("message-timeout")
                .unwrap()
                .parse::<u64>()?;
            let reconnect_delay = test_matches
                .get_one::<String>("reconnect-delay")
                .unwrap()
                .parse::<u64>()?;

            let tester = LoadTester::new(
                url,
                connections,
                interval,
                connection_timeout,
                message_timeout,
                reconnect_delay,
            );
            tester.run().await?;
            Ok(())
        }
        Some(("server", server_matches)) => {
            let port = server_matches.get_one::<String>("port").unwrap().parse::<u16>()?;
            let server = TestServer::new(port);
            server.run().await
        }
        _ => {
            eprintln!("Please specify a subcommand. Use --help for more information.");
            Ok(())
        }
    }
}