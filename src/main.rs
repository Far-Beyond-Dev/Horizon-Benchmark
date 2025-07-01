use clap::{Arg, Command};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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
    // Connection metrics
    connections_attempted: AtomicUsize,
    connections_successful: AtomicUsize,
    connections_failed: AtomicUsize,
    connections_active: AtomicUsize,
    connections_server_kicked: AtomicUsize,
    connections_unexpected_close: AtomicUsize,
    connections_reconnected: AtomicUsize,
    
    // Message metrics
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    messages_failed: AtomicU64,
    messages_timeout: AtomicU64,
    
    // Packet-level metrics
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    
    // Frame type metrics
    text_frames_sent: AtomicU64,
    text_frames_received: AtomicU64,
    binary_frames_sent: AtomicU64,
    binary_frames_received: AtomicU64,
    ping_frames_sent: AtomicU64,
    ping_frames_received: AtomicU64,
    pong_frames_sent: AtomicU64,
    pong_frames_received: AtomicU64,
    close_frames_sent: AtomicU64,
    close_frames_received: AtomicU64,
    
    // Latency metrics
    total_latency_ms: AtomicU64,
    max_latency_ms: AtomicU64,
    min_latency_ms: AtomicU64,
    
    // Latency distribution (percentiles approximation)
    latency_buckets_1ms: AtomicU64,   // 0-1ms
    latency_buckets_10ms: AtomicU64,  // 1-10ms
    latency_buckets_50ms: AtomicU64,  // 10-50ms
    latency_buckets_100ms: AtomicU64, // 50-100ms
    latency_buckets_500ms: AtomicU64, // 100-500ms
    latency_buckets_1s: AtomicU64,    // 500ms-1s
    latency_buckets_5s: AtomicU64,    // 1s-5s
    latency_buckets_over: AtomicU64,  // >5s
    
    // Error metrics
    connection_errors: AtomicU64,
    send_errors: AtomicU64,
    receive_errors: AtomicU64,
    timeout_errors: AtomicU64,
    protocol_errors: AtomicU64,
    
    // Network metrics
    handshake_time_total_ms: AtomicU64,
    handshake_count: AtomicU64,
    
    // Port statistics - use Mutex for HashMap since it's not accessed super frequently
    local_ports: Arc<Mutex<HashMap<u16, u64>>>,   // port -> connection count
    remote_ports: Arc<Mutex<HashMap<u16, u64>>>,  // port -> connection count
    
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
            messages_failed: AtomicU64::new(self.messages_failed.load(Ordering::Relaxed)),
            messages_timeout: AtomicU64::new(self.messages_timeout.load(Ordering::Relaxed)),
            packets_sent: AtomicU64::new(self.packets_sent.load(Ordering::Relaxed)),
            packets_received: AtomicU64::new(self.packets_received.load(Ordering::Relaxed)),
            bytes_sent: AtomicU64::new(self.bytes_sent.load(Ordering::Relaxed)),
            bytes_received: AtomicU64::new(self.bytes_received.load(Ordering::Relaxed)),
            text_frames_sent: AtomicU64::new(self.text_frames_sent.load(Ordering::Relaxed)),
            text_frames_received: AtomicU64::new(self.text_frames_received.load(Ordering::Relaxed)),
            binary_frames_sent: AtomicU64::new(self.binary_frames_sent.load(Ordering::Relaxed)),
            binary_frames_received: AtomicU64::new(self.binary_frames_received.load(Ordering::Relaxed)),
            ping_frames_sent: AtomicU64::new(self.ping_frames_sent.load(Ordering::Relaxed)),
            ping_frames_received: AtomicU64::new(self.ping_frames_received.load(Ordering::Relaxed)),
            pong_frames_sent: AtomicU64::new(self.pong_frames_sent.load(Ordering::Relaxed)),
            pong_frames_received: AtomicU64::new(self.pong_frames_received.load(Ordering::Relaxed)),
            close_frames_sent: AtomicU64::new(self.close_frames_sent.load(Ordering::Relaxed)),
            close_frames_received: AtomicU64::new(self.close_frames_received.load(Ordering::Relaxed)),
            total_latency_ms: AtomicU64::new(self.total_latency_ms.load(Ordering::Relaxed)),
            max_latency_ms: AtomicU64::new(self.max_latency_ms.load(Ordering::Relaxed)),
            min_latency_ms: AtomicU64::new(self.min_latency_ms.load(Ordering::Relaxed)),
            latency_buckets_1ms: AtomicU64::new(self.latency_buckets_1ms.load(Ordering::Relaxed)),
            latency_buckets_10ms: AtomicU64::new(self.latency_buckets_10ms.load(Ordering::Relaxed)),
            latency_buckets_50ms: AtomicU64::new(self.latency_buckets_50ms.load(Ordering::Relaxed)),
            latency_buckets_100ms: AtomicU64::new(self.latency_buckets_100ms.load(Ordering::Relaxed)),
            latency_buckets_500ms: AtomicU64::new(self.latency_buckets_500ms.load(Ordering::Relaxed)),
            latency_buckets_1s: AtomicU64::new(self.latency_buckets_1s.load(Ordering::Relaxed)),
            latency_buckets_5s: AtomicU64::new(self.latency_buckets_5s.load(Ordering::Relaxed)),
            latency_buckets_over: AtomicU64::new(self.latency_buckets_over.load(Ordering::Relaxed)),
            connection_errors: AtomicU64::new(self.connection_errors.load(Ordering::Relaxed)),
            send_errors: AtomicU64::new(self.send_errors.load(Ordering::Relaxed)),
            receive_errors: AtomicU64::new(self.receive_errors.load(Ordering::Relaxed)),
            timeout_errors: AtomicU64::new(self.timeout_errors.load(Ordering::Relaxed)),
            protocol_errors: AtomicU64::new(self.protocol_errors.load(Ordering::Relaxed)),
            handshake_time_total_ms: AtomicU64::new(self.handshake_time_total_ms.load(Ordering::Relaxed)),
            handshake_count: AtomicU64::new(self.handshake_count.load(Ordering::Relaxed)),
            local_ports: Arc::new(Mutex::new(self.local_ports.lock().unwrap().clone())),
            remote_ports: Arc::new(Mutex::new(self.remote_ports.lock().unwrap().clone())),
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
            messages_failed: AtomicU64::new(0),
            messages_timeout: AtomicU64::new(0),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            text_frames_sent: AtomicU64::new(0),
            text_frames_received: AtomicU64::new(0),
            binary_frames_sent: AtomicU64::new(0),
            binary_frames_received: AtomicU64::new(0),
            ping_frames_sent: AtomicU64::new(0),
            ping_frames_received: AtomicU64::new(0),
            pong_frames_sent: AtomicU64::new(0),
            pong_frames_received: AtomicU64::new(0),
            close_frames_sent: AtomicU64::new(0),
            close_frames_received: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            max_latency_ms: AtomicU64::new(0),
            min_latency_ms: AtomicU64::new(u64::MAX),
            latency_buckets_1ms: AtomicU64::new(0),
            latency_buckets_10ms: AtomicU64::new(0),
            latency_buckets_50ms: AtomicU64::new(0),
            latency_buckets_100ms: AtomicU64::new(0),
            latency_buckets_500ms: AtomicU64::new(0),
            latency_buckets_1s: AtomicU64::new(0),
            latency_buckets_5s: AtomicU64::new(0),
            latency_buckets_over: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
            send_errors: AtomicU64::new(0),
            receive_errors: AtomicU64::new(0),
            timeout_errors: AtomicU64::new(0),
            protocol_errors: AtomicU64::new(0),
            handshake_time_total_ms: AtomicU64::new(0),
            handshake_count: AtomicU64::new(0),
            local_ports: Arc::new(Mutex::new(HashMap::new())),
            remote_ports: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    fn record_connection_ports(&self, local_port: u16, remote_port: u16) {
        if let Ok(mut local_ports) = self.local_ports.lock() {
            *local_ports.entry(local_port).or_insert(0) += 1;
        }
        if let Ok(mut remote_ports) = self.remote_ports.lock() {
            *remote_ports.entry(remote_port).or_insert(0) += 1;
        }
    }

    fn record_message_sent(&self, message: &Message) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        
        match message {
            Message::Text(text) => {
                self.text_frames_sent.fetch_add(1, Ordering::Relaxed);
                self.bytes_sent.fetch_add(text.len() as u64, Ordering::Relaxed);
            }
            Message::Binary(data) => {
                self.binary_frames_sent.fetch_add(1, Ordering::Relaxed);
                self.bytes_sent.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            Message::Ping(data) => {
                self.ping_frames_sent.fetch_add(1, Ordering::Relaxed);
                self.bytes_sent.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            Message::Pong(data) => {
                self.pong_frames_sent.fetch_add(1, Ordering::Relaxed);
                self.bytes_sent.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            Message::Close(_) => {
                self.close_frames_sent.fetch_add(1, Ordering::Relaxed);
                self.bytes_sent.fetch_add(2, Ordering::Relaxed); // Close frame typically 2 bytes
            }
            Message::Frame(_) => {
                // Raw frame - estimate size
                self.bytes_sent.fetch_add(4, Ordering::Relaxed);
            }
        }
    }

    fn record_message_received(&self, message: &Message) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
        
        match message {
            Message::Text(text) => {
                self.text_frames_received.fetch_add(1, Ordering::Relaxed);
                self.bytes_received.fetch_add(text.len() as u64, Ordering::Relaxed);
            }
            Message::Binary(data) => {
                self.binary_frames_received.fetch_add(1, Ordering::Relaxed);
                self.bytes_received.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            Message::Ping(data) => {
                self.ping_frames_received.fetch_add(1, Ordering::Relaxed);
                self.bytes_received.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            Message::Pong(data) => {
                self.pong_frames_received.fetch_add(1, Ordering::Relaxed);
                self.bytes_received.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            Message::Close(_) => {
                self.close_frames_received.fetch_add(1, Ordering::Relaxed);
                self.bytes_received.fetch_add(2, Ordering::Relaxed);
            }
            Message::Frame(_) => {
                self.bytes_received.fetch_add(4, Ordering::Relaxed);
            }
        }
    }

    fn record_latency(&self, latency_ms: u64) {
        self.total_latency_ms.fetch_add(latency_ms, Ordering::Relaxed);
        
        // Update latency buckets
        if latency_ms <= 1 {
            self.latency_buckets_1ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ms <= 10 {
            self.latency_buckets_10ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ms <= 50 {
            self.latency_buckets_50ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ms <= 100 {
            self.latency_buckets_100ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ms <= 500 {
            self.latency_buckets_500ms.fetch_add(1, Ordering::Relaxed);
        } else if latency_ms <= 1000 {
            self.latency_buckets_1s.fetch_add(1, Ordering::Relaxed);
        } else if latency_ms <= 5000 {
            self.latency_buckets_5s.fetch_add(1, Ordering::Relaxed);
        } else {
            self.latency_buckets_over.fetch_add(1, Ordering::Relaxed);
        }
        
        // Update max latency
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

        // Update min latency
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

    fn record_handshake_time(&self, duration_ms: u64) {
        self.handshake_count.fetch_add(1, Ordering::Relaxed);
        self.handshake_time_total_ms.fetch_add(duration_ms, Ordering::Relaxed);
    }

    fn print_stats(&self) {
        let attempted = self.connections_attempted.load(Ordering::Relaxed);
        let successful = self.connections_successful.load(Ordering::Relaxed);
        let failed = self.connections_failed.load(Ordering::Relaxed);
        let active = self.connections_active.load(Ordering::Relaxed);
        let server_kicked = self.connections_server_kicked.load(Ordering::Relaxed);
        let unexpected_close = self.connections_unexpected_close.load(Ordering::Relaxed);
        let reconnected = self.connections_reconnected.load(Ordering::Relaxed);
        
        let msg_sent = self.messages_sent.load(Ordering::Relaxed);
        let msg_received = self.messages_received.load(Ordering::Relaxed);
        let msg_failed = self.messages_failed.load(Ordering::Relaxed);
        let msg_timeout = self.messages_timeout.load(Ordering::Relaxed);
        
        let packets_sent = self.packets_sent.load(Ordering::Relaxed);
        let packets_received = self.packets_received.load(Ordering::Relaxed);
        let bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        let bytes_received = self.bytes_received.load(Ordering::Relaxed);
        
        let text_sent = self.text_frames_sent.load(Ordering::Relaxed);
        let text_received = self.text_frames_received.load(Ordering::Relaxed);
        let binary_sent = self.binary_frames_sent.load(Ordering::Relaxed);
        let binary_received = self.binary_frames_received.load(Ordering::Relaxed);
        let ping_sent = self.ping_frames_sent.load(Ordering::Relaxed);
        let ping_received = self.ping_frames_received.load(Ordering::Relaxed);
        let pong_sent = self.pong_frames_sent.load(Ordering::Relaxed);
        let pong_received = self.pong_frames_received.load(Ordering::Relaxed);
        let close_sent = self.close_frames_sent.load(Ordering::Relaxed);
        let close_received = self.close_frames_received.load(Ordering::Relaxed);
        
        let total_latency = self.total_latency_ms.load(Ordering::Relaxed);
        let max_latency = self.max_latency_ms.load(Ordering::Relaxed);
        let min_latency = self.min_latency_ms.load(Ordering::Relaxed);
        
        let handshake_total = self.handshake_time_total_ms.load(Ordering::Relaxed);
        let handshake_count = self.handshake_count.load(Ordering::Relaxed);
        
        let conn_errors = self.connection_errors.load(Ordering::Relaxed);
        let send_errors = self.send_errors.load(Ordering::Relaxed);
        let recv_errors = self.receive_errors.load(Ordering::Relaxed);
        let timeout_errors = self.timeout_errors.load(Ordering::Relaxed);
        let protocol_errors = self.protocol_errors.load(Ordering::Relaxed);

        let avg_latency = if msg_received > 0 {
            total_latency / msg_received
        } else {
            0
        };

        let avg_handshake = if handshake_count > 0 {
            handshake_total / handshake_count
        } else {
            0
        };

        let min_display = if min_latency == u64::MAX { 0 } else { min_latency };
        let uptime = self.start_time.elapsed();
        let uptime_secs = uptime.as_secs_f64();

        info!("=== WebSocket Test Statistics ===");
        info!("Test uptime: {:.2} hours ({:.0} seconds)", uptime_secs / 3600.0, uptime_secs);
        
        info!("--- CONNECTION METRICS ---");
        info!("Connections attempted: {} ({:.2}/sec)", attempted, if uptime_secs > 0.0 { attempted as f64 / uptime_secs } else { 0.0 });
        info!("Connections successful: {} ({:.2}/sec)", successful, if uptime_secs > 0.0 { successful as f64 / uptime_secs } else { 0.0 });
        info!("Connections active: {}", active);
        info!("Connections failed: {} ({:.2}/sec)", failed, if uptime_secs > 0.0 { failed as f64 / uptime_secs } else { 0.0 });
        info!("Connections server-kicked: {} ({:.2}/sec)", server_kicked, if uptime_secs > 0.0 { server_kicked as f64 / uptime_secs } else { 0.0 });
        info!("Connections unexpected close: {} ({:.2}/sec)", unexpected_close, if uptime_secs > 0.0 { unexpected_close as f64 / uptime_secs } else { 0.0 });
        info!("Connections reconnected: {} ({:.2}/sec)", reconnected, if uptime_secs > 0.0 { reconnected as f64 / uptime_secs } else { 0.0 });
        info!("Success rate: {:.2}%", if attempted > 0 { (successful as f64 / attempted as f64) * 100.0 } else { 0.0 });
        info!("Average handshake time: {} ms", avg_handshake);
        
        info!("--- MESSAGE METRICS ---");
        info!("Messages sent: {} ({:.2}/sec)", msg_sent, if uptime_secs > 0.0 { msg_sent as f64 / uptime_secs } else { 0.0 });
        info!("Messages received: {} ({:.2}/sec)", msg_received, if uptime_secs > 0.0 { msg_received as f64 / uptime_secs } else { 0.0 });
        info!("Messages failed: {} ({:.2}/sec)", msg_failed, if uptime_secs > 0.0 { msg_failed as f64 / uptime_secs } else { 0.0 });
        info!("Messages timeout: {} ({:.2}/sec)", msg_timeout, if uptime_secs > 0.0 { msg_timeout as f64 / uptime_secs } else { 0.0 });
        info!("Message success rate: {:.2}%", if msg_sent > 0 { (msg_received as f64 / msg_sent as f64) * 100.0 } else { 0.0 });
        info!("Messages/min sent: {:.2}", if uptime_secs > 0.0 { msg_sent as f64 / (uptime_secs / 60.0) } else { 0.0 });
        info!("Messages/min received: {:.2}", if uptime_secs > 0.0 { msg_received as f64 / (uptime_secs / 60.0) } else { 0.0 });
        
        info!("--- PACKET/BANDWIDTH METRICS ---");
        info!("Packets sent: {} ({:.2}/sec)", packets_sent, if uptime_secs > 0.0 { packets_sent as f64 / uptime_secs } else { 0.0 });
        info!("Packets received: {} ({:.2}/sec)", packets_received, if uptime_secs > 0.0 { packets_received as f64 / uptime_secs } else { 0.0 });
        info!("Bytes sent: {} ({:.2} MB, {:.2} KB/s)", bytes_sent, bytes_sent as f64 / 1_048_576.0, if uptime_secs > 0.0 { bytes_sent as f64 / uptime_secs / 1024.0 } else { 0.0 });
        info!("Bytes received: {} ({:.2} MB, {:.2} KB/s)", bytes_received, bytes_received as f64 / 1_048_576.0, if uptime_secs > 0.0 { bytes_received as f64 / uptime_secs / 1024.0 } else { 0.0 });
        
        info!("--- FRAME TYPE BREAKDOWN ---");
        info!("Text frames sent/received: {} / {} ({:.2}/sec / {:.2}/sec)", 
              text_sent, text_received,
              if uptime_secs > 0.0 { text_sent as f64 / uptime_secs } else { 0.0 },
              if uptime_secs > 0.0 { text_received as f64 / uptime_secs } else { 0.0 });
        info!("Binary frames sent/received: {} / {} ({:.2}/sec / {:.2}/sec)", 
              binary_sent, binary_received,
              if uptime_secs > 0.0 { binary_sent as f64 / uptime_secs } else { 0.0 },
              if uptime_secs > 0.0 { binary_received as f64 / uptime_secs } else { 0.0 });
        info!("Ping frames sent/received: {} / {} ({:.2}/sec / {:.2}/sec)", 
              ping_sent, ping_received,
              if uptime_secs > 0.0 { ping_sent as f64 / uptime_secs } else { 0.0 },
              if uptime_secs > 0.0 { ping_received as f64 / uptime_secs } else { 0.0 });
        info!("Pong frames sent/received: {} / {} ({:.2}/sec / {:.2}/sec)", 
              pong_sent, pong_received,
              if uptime_secs > 0.0 { pong_sent as f64 / uptime_secs } else { 0.0 },
              if uptime_secs > 0.0 { pong_received as f64 / uptime_secs } else { 0.0 });
        info!("Close frames sent/received: {} / {} ({:.2}/sec / {:.2}/sec)", 
              close_sent, close_received,
              if uptime_secs > 0.0 { close_sent as f64 / uptime_secs } else { 0.0 },
              if uptime_secs > 0.0 { close_received as f64 / uptime_secs } else { 0.0 });
        
        info!("--- LATENCY METRICS ---");
        info!("Average latency: {} ms", avg_latency);
        info!("Min latency: {} ms", min_display);
        info!("Max latency: {} ms", max_latency);
        
        // Latency distribution
        let bucket_1ms = self.latency_buckets_1ms.load(Ordering::Relaxed);
        let bucket_10ms = self.latency_buckets_10ms.load(Ordering::Relaxed);
        let bucket_50ms = self.latency_buckets_50ms.load(Ordering::Relaxed);
        let bucket_100ms = self.latency_buckets_100ms.load(Ordering::Relaxed);
        let bucket_500ms = self.latency_buckets_500ms.load(Ordering::Relaxed);
        let bucket_1s = self.latency_buckets_1s.load(Ordering::Relaxed);
        let bucket_5s = self.latency_buckets_5s.load(Ordering::Relaxed);
        let bucket_over = self.latency_buckets_over.load(Ordering::Relaxed);
        let total_samples = bucket_1ms + bucket_10ms + bucket_50ms + bucket_100ms + bucket_500ms + bucket_1s + bucket_5s + bucket_over;
        
        if total_samples > 0 {
            info!("--- LATENCY DISTRIBUTION ---");
            info!("0-1ms: {} ({:.1}%)", bucket_1ms, (bucket_1ms as f64 / total_samples as f64) * 100.0);
            info!("1-10ms: {} ({:.1}%)", bucket_10ms, (bucket_10ms as f64 / total_samples as f64) * 100.0);
            info!("10-50ms: {} ({:.1}%)", bucket_50ms, (bucket_50ms as f64 / total_samples as f64) * 100.0);
            info!("50-100ms: {} ({:.1}%)", bucket_100ms, (bucket_100ms as f64 / total_samples as f64) * 100.0);
            info!("100-500ms: {} ({:.1}%)", bucket_500ms, (bucket_500ms as f64 / total_samples as f64) * 100.0);
            info!("0.5-1s: {} ({:.1}%)", bucket_1s, (bucket_1s as f64 / total_samples as f64) * 100.0);
            info!("1-5s: {} ({:.1}%)", bucket_5s, (bucket_5s as f64 / total_samples as f64) * 100.0);
            info!(">5s: {} ({:.1}%)", bucket_over, (bucket_over as f64 / total_samples as f64) * 100.0);
        }
        
        info!("--- ERROR METRICS ---");
        info!("Connection errors: {} ({:.2}/sec)", conn_errors, if uptime_secs > 0.0 { conn_errors as f64 / uptime_secs } else { 0.0 });
        info!("Send errors: {} ({:.2}/sec)", send_errors, if uptime_secs > 0.0 { send_errors as f64 / uptime_secs } else { 0.0 });
        info!("Receive errors: {} ({:.2}/sec)", recv_errors, if uptime_secs > 0.0 { recv_errors as f64 / uptime_secs } else { 0.0 });
        info!("Timeout errors: {} ({:.2}/sec)", timeout_errors, if uptime_secs > 0.0 { timeout_errors as f64 / uptime_secs } else { 0.0 });
        info!("Protocol errors: {} ({:.2}/sec)", protocol_errors, if uptime_secs > 0.0 { protocol_errors as f64 / uptime_secs } else { 0.0 });
        let total_errors = conn_errors + send_errors + recv_errors + timeout_errors + protocol_errors;
        info!("Total errors: {} ({:.2}/sec)", total_errors, if uptime_secs > 0.0 { total_errors as f64 / uptime_secs } else { 0.0 });
        
        // Port statistics
        info!("--- PORT STATISTICS ---");
        if let Ok(local_ports) = self.local_ports.lock() {
            let total_local_connections: u64 = local_ports.values().sum();
            let unique_local_ports = local_ports.len();
            info!("Unique local ports used: {} (total connections: {})", unique_local_ports, total_local_connections);
            if !local_ports.is_empty() {
                let mut sorted_local: Vec<_> = local_ports.iter().collect();
                sorted_local.sort_by(|a, b| b.1.cmp(a.1));
                info!("Top local ports:");
                for (port, count) in sorted_local.iter().take(10) {
                    info!("  Port {}: {} connections ({:.1}%)", port, count, (**count as f64 / total_local_connections as f64) * 100.0);
                }
            }
        }
        
        if let Ok(remote_ports) = self.remote_ports.lock() {
            let total_remote_connections: u64 = remote_ports.values().sum();
            let unique_remote_ports = remote_ports.len();
            info!("Unique remote ports used: {} (total connections: {})", unique_remote_ports, total_remote_connections);
            if !remote_ports.is_empty() {
                let mut sorted_remote: Vec<_> = remote_ports.iter().collect();
                sorted_remote.sort_by(|a, b| b.1.cmp(a.1));
                info!("Top remote ports:");
                for (port, count) in sorted_remote.iter().take(10) {
                    info!("  Port {}: {} connections ({:.1}%)", port, count, (**count as f64 / total_remote_connections as f64) * 100.0);
                }
            }
        }
        
        // Calculate disconnect rates
        let total_disconnects = server_kicked + unexpected_close;
        if total_disconnects > 0 {
            warn!("--- DISCONNECT ANALYSIS ---");
            warn!("Total disconnects: {} ({:.2}/sec)", total_disconnects, if uptime_secs > 0.0 { total_disconnects as f64 / uptime_secs } else { 0.0 });
            warn!("Server kicks: {} ({:.2}%, {:.2}/sec)", server_kicked, (server_kicked as f64 / total_disconnects as f64) * 100.0, if uptime_secs > 0.0 { server_kicked as f64 / uptime_secs } else { 0.0 });
            warn!("Unexpected closes: {} ({:.2}%, {:.2}/sec)", unexpected_close, (unexpected_close as f64 / total_disconnects as f64) * 100.0, if uptime_secs > 0.0 { unexpected_close as f64 / uptime_secs } else { 0.0 });
            warn!("Disconnect rate: {:.4}/hour", total_disconnects as f64 / (uptime_secs / 3600.0));
        }
    }

    fn print_final_stats(&self) {
        info!("\n{}", "=".repeat(60));
        info!("                    FINAL TEST RESULTS");
        info!("{}", "=".repeat(60));
        self.print_stats();
        info!("{}", "=".repeat(60));
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

            let (ws_stream, local_addr, remote_addr) = match connect_result {
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

                    (stream, local_addr, remote_addr)
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
        .version("3.0")
        .author("Enhanced Load Tester")
        .about("High-performance endless WebSocket load testing tool with comprehensive metrics")
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
                        .help("Interval between messages in milliseconds (40ms = 1500 msgs/min)")
                        .default_value("40"),
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