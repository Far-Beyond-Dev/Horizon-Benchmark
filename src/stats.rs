use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{info, warn};

#[derive(Debug)]
pub struct TestStats {
    // Connection metrics
    pub connections_attempted: AtomicUsize,
    pub connections_successful: AtomicUsize,
    pub connections_failed: AtomicUsize,
    pub connections_active: AtomicUsize,
    pub connections_server_kicked: AtomicUsize,
    pub connections_unexpected_close: AtomicUsize,
    pub connections_reconnected: AtomicUsize,
    
    // Message metrics
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub messages_failed: AtomicU64,
    pub messages_timeout: AtomicU64,
    
    // Packet-level metrics
    pub packets_sent: AtomicU64,
    pub packets_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    
    // Frame type metrics
    pub text_frames_sent: AtomicU64,
    pub text_frames_received: AtomicU64,
    pub binary_frames_sent: AtomicU64,
    pub binary_frames_received: AtomicU64,
    pub ping_frames_sent: AtomicU64,
    pub ping_frames_received: AtomicU64,
    pub pong_frames_sent: AtomicU64,
    pub pong_frames_received: AtomicU64,
    pub close_frames_sent: AtomicU64,
    pub close_frames_received: AtomicU64,
    
    // Latency metrics
    pub total_latency_ms: AtomicU64,
    pub max_latency_ms: AtomicU64,
    pub min_latency_ms: AtomicU64,
    
    // Latency distribution (percentiles approximation)
    pub latency_buckets_1ms: AtomicU64,   // 0-1ms
    pub latency_buckets_10ms: AtomicU64,  // 1-10ms
    pub latency_buckets_50ms: AtomicU64,  // 10-50ms
    pub latency_buckets_100ms: AtomicU64, // 50-100ms
    pub latency_buckets_500ms: AtomicU64, // 100-500ms
    pub latency_buckets_1s: AtomicU64,    // 500ms-1s
    pub latency_buckets_5s: AtomicU64,    // 1s-5s
    pub latency_buckets_over: AtomicU64,  // >5s
    
    // Error metrics
    pub connection_errors: AtomicU64,
    pub send_errors: AtomicU64,
    pub receive_errors: AtomicU64,
    pub timeout_errors: AtomicU64,
    pub protocol_errors: AtomicU64,
    
    // Network metrics
    pub handshake_time_total_ms: AtomicU64,
    pub handshake_count: AtomicU64,
    
    // Port statistics - use Mutex for HashMap since it's not accessed super frequently
    pub local_ports: Arc<Mutex<HashMap<u16, u64>>>,   // port -> connection count
    pub remote_ports: Arc<Mutex<HashMap<u16, u64>>>,  // port -> connection count
    
    pub start_time: Instant,
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
    pub fn new() -> Self {
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

    pub fn record_connection_ports(&self, local_port: u16, remote_port: u16) {
        if let Ok(mut local_ports) = self.local_ports.lock() {
            *local_ports.entry(local_port).or_insert(0) += 1;
        }
        if let Ok(mut remote_ports) = self.remote_ports.lock() {
            *remote_ports.entry(remote_port).or_insert(0) += 1;
        }
    }

    pub fn record_message_sent(&self, message: &Message) {
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

    pub fn record_message_received(&self, message: &Message) {
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

    pub fn record_latency(&self, latency_ms: u64) {
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

    pub fn record_handshake_time(&self, duration_ms: u64) {
        self.handshake_count.fetch_add(1, Ordering::Relaxed);
        self.handshake_time_total_ms.fetch_add(duration_ms, Ordering::Relaxed);
    }

    pub fn print_stats(&self) {
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

    pub fn print_final_stats(&self) {
        info!("\n{}", "=".repeat(60));
        info!("                    FINAL TEST RESULTS");
        info!("{}", "=".repeat(60));
        self.print_stats();
        info!("{}", "=".repeat(60));
    }
}