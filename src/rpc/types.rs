use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<Value>,
    pub id: Value,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddRpcRequest {
    pub url: String,
    pub ws_url: String,
    pub chain_id: usize,
    pub rpc_location: String,
}

pub enum RpcRequest {
    JsonRpc(JsonRpcRequest),
    AddRpc(AddRpcRequest),
    JsonRpcArray(Vec<JsonRpcRequest>),
    AddRpcArray(Vec<AddRpcRequest>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RpcLocation {
    Local,
    External,
}


impl FromStr for RpcLocation {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(RpcLocation::Local),
            "external" => Ok(RpcLocation::External),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct JsonRpcResponse {
    pub id: Value,
    pub jsonrpc: String,
    pub result: String,
}

impl JsonRpcResponse {
    pub fn new(result: String) -> Self {
        Self {
            id: Value::from(1),
            jsonrpc: "2.0".to_string(),
            result,
        }
    }

    pub fn to_json(&self) -> String {
        json!({
            "id": &self.id,
            "jsonrpc": &self.jsonrpc,
            "result": &self.result,
        })
            .to_string()
    }
}

impl From<String> for JsonRpcResponse {
    fn from(result: String) -> Self {
        Self::new(result)
    }
}


impl Default for RpcLocation {
    fn default() -> Self {
        RpcLocation::Local
    }
}

#[derive(Debug, Clone)]
pub struct Rpc {
    pub url: String,               // url of the rpc
    pub ws_url: String,            // url of the websocket to keep track of the latest block
    pub chain_id: usize,           // id for chain_id, ethereum = 1, optimism = 10, base = 8453
    pub rpc_location: RpcLocation, // location of the rpc, local or external
    pub last_block: u64,           // blockchain last block number
    pub last_block_ts: u64,        // timestamp of the last block
    pub current_ts: u64,           // Arrival last block to calculate the latency
    pub avg_latency: f64,          // average latency of the rpc
    pub intra_latencies: LimitedVecDeque, // n last intra latencies of the rpc
    pub srv_latencies: LimitedVecDeque,   // n last srv latencies of the rpc
    pub arrivals_ts: LimitedVecDeque,
}

impl PartialEq for Rpc {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl Default for Rpc {
    fn default() -> Self {
        Self {
            url: "".to_string(),
            ws_url: "".to_string(),
            chain_id: 0,
            rpc_location: RpcLocation::Local,
            last_block: 0,
            last_block_ts: 0,
            current_ts: 0,
            avg_latency: 0.0,
            intra_latencies: LimitedVecDeque::new(1000),
            srv_latencies: LimitedVecDeque::new(1000),
            arrivals_ts: LimitedVecDeque::new(1000),
        }
    }
}


impl Rpc {
    pub async fn new(
        url: String,
        ws_url: String,
        chain_id: usize,
        rpc_location: RpcLocation,
        stats_vec_size: usize,
    ) -> Self {
        // Return the Rpc struct
        Self {
            url,
            ws_url,
            chain_id,
            rpc_location,
            last_block: 0,
            last_block_ts: 0,
            current_ts: 0,
            avg_latency: 0.0,
            intra_latencies: LimitedVecDeque::new(stats_vec_size),
            srv_latencies: LimitedVecDeque::new(stats_vec_size),
            arrivals_ts: LimitedVecDeque::new(stats_vec_size),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitedVecDeque {
    pub deque: VecDeque<u64>,
    pub limit: usize,
}

impl LimitedVecDeque {
    pub fn new(limit: usize) -> Self {
        Self {
            deque: VecDeque::new(),
            limit,
        }
    }

    pub fn push(&mut self, value: u64) {
        if self.deque.len() == self.limit {
            self.deque.pop_back();
        }
        self.deque.push_front(value);
    }
}


