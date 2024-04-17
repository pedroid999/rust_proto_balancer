mod config;
mod rpc;
mod sort;
mod websocket;

use crate::{
    config::types::Settings,
    websocket::types::RpcWebSocket,
    rpc::functions::forward_json_rpc_request,
};

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::{RwLock};
use env_logger::Builder;
use log::{error, info};
use log::LevelFilter;
use std::str::FromStr;
use env_logger::TimestampPrecision::Millis;
// NOTES
// logger should not use the lib name but a global process name (load_balancer_{url})
// logger need to print miliseconds timestamps
// config is not persisted
// how to fetch latency stats (endpoint?)
use lazy_static::lazy_static;
use reqwest::Client;

lazy_static! {
    static ref CLIENT: Client = Client::new();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    // Get all the cli args and set them
    let config = Arc::new(RwLock::new(Settings::new(Settings::create_match()).await));

    // Copy the configuration values we need
    let (addr, log_level, stat_vec_size, algo) = {
        let config_guard = config.read().await;
        (config_guard.address, config_guard.log_level.clone(), config_guard.stats_vec_size, config_guard.algo.clone())

    };

    Builder::new()
        .filter_level(LevelFilter::from_str(log_level.as_str()).unwrap()) // Set the log level
        .write_style(env_logger::WriteStyle::Always) // Enable output to stdout
        .format_timestamp(Some(Millis))
        .init();

    // Make a mutex rpc list
    let rpc_list_rwlock = Arc::new(Mutex::new(config.read().await.rpc_list.clone()));

    // start all rpc websockets with tokio::task
    let len = rpc_list_rwlock.lock().unwrap().len();
    for index in 0..len {
        let rpc_list_rwlock_clone = rpc_list_rwlock.clone();
        let rpc_websocket =
            RpcWebSocket::new(rpc_list_rwlock_clone.lock().unwrap()[index].ws_url.clone());
        let mut rpc_websocket_clone = rpc_websocket.await.clone();
        info!("Starting web sockets {}", index);
        tokio::task::spawn(async move {
            rpc_websocket_clone
                .start_rpc(rpc_list_rwlock_clone, index)
                .await;
        });
    }

    let listener = TcpListener::bind(addr).await?;

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;
        // Use and adapter to access something implementing 'tokio::io' traits as if they implement
        // 'hyper::rt' IO traits.
        let io = TokioIo::new(stream);
        let rpc_list_rwlock_clone = rpc_list_rwlock.clone();
        let algo = algo.clone();
        // Spawn a tokio task to serve multiple connections concurrently.
        tokio::task::spawn(async move {
            let rpc_list_rwlock_clone = rpc_list_rwlock_clone.clone();
            let algo = algo.clone();
            let start = std::time::Instant::now();
            // Finally, we bind the incoming connection to our 'forward_json_rpc_request' service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(
                    io,
                    service_fn(|req| {
                        let rpc_list_rwlock_clone = rpc_list_rwlock_clone.clone();
                        let algo = algo.clone();
                        forward_json_rpc_request(req, rpc_list_rwlock_clone, stat_vec_size, algo)
                    }),
                )
                .with_upgrades()
                .await
            {
                error!("Error serving connection: {}", err);
            }
            info!("Request took: {:?}", start.elapsed());
        });
    }
}
