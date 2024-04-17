use crate::{
    rpc::types::{
        Rpc,
        JsonRpcRequest,
    }
};

use futures_util::sink::SinkExt;
use futures_util::StreamExt;
use serde_json::{Value};
use std::sync::{Arc, Mutex};
use log::{debug, error, info};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream};

#[derive(Debug, Clone)]
pub struct RpcWebSocket {
    pub ws_url: String, // url of the rpc
    pub ws_stream: Arc<RwLock<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
}

impl Default for RpcWebSocket {
    fn default() -> Self {
        Self {
            ws_url: "".to_string(),
            ws_stream: Arc::new(RwLock::new(None)),
        }
    }
}

impl RpcWebSocket {
    pub async fn new(ws_url: String) -> Self {
        info!("Creating new WS connection to: {}", ws_url);
        // Connect to the websocket
        let (ws_stream, _) = connect_async(&ws_url).await.expect("Failed to connect");

        // Return the Rpc struct
        Self {
            ws_url,
            ws_stream: Arc::new(RwLock::new(Some(ws_stream))),
        }
    }

    pub async fn subscribe_to_new_heads(&self) {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_subscribe".to_string(),
            params: vec![serde_json::Value::String("newHeads".to_string())],
            id: Value::Null,
        };

        let request_json = serde_json::to_string(&request).unwrap();

        let mut ws_stream_guard = self.ws_stream.write().await;
        if let Some(ws_stream) = ws_stream_guard.as_mut() {
            ws_stream.send(Message::Text(request_json)).await.unwrap();
        }
    }

    pub async fn start_rpc(&mut self, rpc_list: Arc<Mutex<Vec<Rpc>>>, index_rpc: usize) {
        self.subscribe_to_new_heads().await;
        let mut cloned_self = self.clone();
        tokio::task::spawn(async move { cloned_self.listen_for_updates(rpc_list, index_rpc).await });
    }

    pub async fn listen_for_updates(&mut self, rpc_list: Arc<Mutex<Vec<Rpc>>>, index_rpc: usize) {
        loop {
            // Get the websocket stream
            let mut ws_stream = self.ws_stream.write().await;
            if let Some(stream) = ws_stream.as_mut() {
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(msg) => {
                            self.process_message(msg.clone(), rpc_list.clone(), index_rpc).await;
                        }
                        Err(e) => error!("Error reading message: {}", e),
                    }
                }
            }
        }
    }

    pub async fn process_message(&self, msg: Message, rpc_list: Arc<Mutex<Vec<Rpc>>>, index_rpc: usize) {
        match msg.is_text() {
            true => {
                let text = msg.into_text().unwrap();
                let value: Value = serde_json::from_str(&text).unwrap();
                self.process_params(&value, rpc_list.clone(), index_rpc).await;
            }
            false => match msg {
                Message::Ping(ping) => debug!("Received Ping: {:?}", ping),
                _ => error!("Message is not text: {:?}", msg),
            }
        }
    }

    pub async fn process_params(&self, value: &Value, rpc_list: Arc<Mutex<Vec<Rpc>>>, index_rpc: usize) {
        if value.get("method").map_or(false, |m| m.as_str() == Some("eth_subscription")) {
            let result = value.get("params").and_then(|p| p.get("result"));

            let block_number = u64::from_str_radix(
                result
                    .and_then(|r| r.get("number"))
                    .and_then(|n| n.as_str())
                    .map_or("", |n| n.trim_start_matches("0x")), 16).unwrap();

            let timestamp = u64::from_str_radix(
                result
                    .and_then(|r| r.get("timestamp"))
                    .and_then(|t| t.as_str())
                    .map_or("", |t| t.trim_start_matches("0x")), 16).unwrap() * 1000;

            let current_timestamp = chrono::Utc::now().timestamp_millis() as u64;
            {
                let mut rpc_guard = rpc_list.lock().unwrap();
                rpc_guard[index_rpc].last_block = block_number;
                rpc_guard[index_rpc].last_block_ts = timestamp;
                rpc_guard[index_rpc].current_ts = current_timestamp;

                debug!("Rpc updated url: {:?}", rpc_guard[index_rpc].url);
                debug!("Rpc updated last block: {:?}", rpc_guard[index_rpc].last_block);
                debug!("Rpc updated last block ts: {:?}", rpc_guard[index_rpc].last_block_ts);
                debug!("Rpc updated current ts: {:?}", rpc_guard[index_rpc].current_ts);
            }
        }
    }
}
