use crate::{rpc::{
    types::{
        AddRpcRequest,
        JsonRpcRequest,
        JsonRpcResponse,
        Rpc,
        RpcLocation,
        RpcRequest,
    },
    errors::{
        ApplicationError,
        ErrorCode,
        JsonRpcErrorResponse,
    },
}, websocket::types::RpcWebSocket, sort::{
    types::Algo,
    functions::sort_rpc_list_by_algo,
}, CLIENT};

use std::io::Error;
use http_body_util::BodyExt;
use hyper::{Request, Response};
use serde_json::Value;
use simd_json::serde::from_str;
use std::str::from_utf8;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use log::{debug, error, info};
use std::str::FromStr;
use futures_util::stream::FuturesUnordered;
use tokio_stream::StreamExt;

pub fn parse_rpc_request(value: Value) -> Result<RpcRequest, serde_json::Error> {
    if let Ok(req) = serde_json::from_value::<JsonRpcRequest>(value.clone()) {
        Ok(RpcRequest::JsonRpc(req))
    } else if let Ok(reqs) = serde_json::from_value::<Vec<JsonRpcRequest>>(value.clone()) {
        Ok(RpcRequest::JsonRpcArray(reqs))
    } else if let Ok(req) = serde_json::from_value::<AddRpcRequest>(value.clone()) {
        Ok(RpcRequest::AddRpc(req))
    } else if let Ok(reqs) = serde_json::from_value::<Vec<AddRpcRequest>>(value.clone()) {
        Ok(RpcRequest::AddRpcArray(reqs))
    } else {
        Err(serde_json::Error::io(Error::new(std::io::ErrorKind::Other, "Unknown request type")))
    }
}

pub async fn forward_json_rpc_request(
    request: Request<hyper::body::Incoming>,
    rpc_list: Arc<Mutex<Vec<Rpc>>>, stat_vec_size: usize, algo: Algo,
) -> Result<Response<String>, hyper::Error> {
    let chain_id = extract_chain_id(request.uri().path());
    let json_value = incoming_to_value(request).await?;

    match parse_rpc_request(json_value.clone()) {
        Ok(RpcRequest::JsonRpc(req)) => {
            if req.method == "eth_sendRawTransaction" {
                forward_raw_transaction(rpc_list, chain_id, json_value.clone()).await
            }
            else{
                forward_rpc_request(rpc_list, chain_id, json_value.clone(), algo).await
            }
        },
        Ok(RpcRequest::JsonRpcArray(_reqs)) => {
            forward_rpc_request(rpc_list, chain_id, json_value.clone(), algo).await
        },
        Ok(RpcRequest::AddRpc(req)) => {
            add_rpc(rpc_list, req, stat_vec_size).await
        },
        Ok(RpcRequest::AddRpcArray(req)) => {
            let mut responses = Vec::new();
            for rpc in req {
                let response = add_rpc(rpc_list.clone(), rpc, stat_vec_size).await?;
                responses.push(response.into_body());
            }
            Ok(Response::new(format!("[{}]", responses.join(","))))
        },
        Err(error) => {
            let json_response = JsonRpcErrorResponse::from(ApplicationError::new(
                ErrorCode::BadRequest,
                error.to_string(),
            ));
            error!("Error: {}", json_response.error.format_error().as_str());
            Ok(Response::new(json_response.to_json()))
        }
    }
}

pub async fn forward_raw_transaction(rpc_list: Arc<Mutex<Vec<Rpc>>>, chain_id: usize,
                                 json_value: Value,
) -> Result<Response<String>, hyper::Error> {

    let rpc_list_copy = {
        let rpc_guard = rpc_list.lock().unwrap();
        // Here only the operation that needs exclusive access to the data is performed.
        // In this case, a copy of the data is being made.
        rpc_guard.clone()
    };

    // Filter the RPCs by chain ID if chain_id is not 0
    let filtered_rpc_list: Vec<Rpc> = {
        let rpc_list_copy_clone: Vec<Rpc> = rpc_list_copy
            .iter()
            .filter(|rpc| rpc.chain_id == chain_id)
            .cloned()
            .collect();
        rpc_list_copy_clone.clone()
    };

    let last_response = {
        let mut futures = FuturesUnordered::new();
        let mut results = Vec::new();
        for rpc in filtered_rpc_list.clone() {
            let json_value_clone = json_value.clone();
            info!("Sending raw transaction {} to: {}", json_value.clone(), rpc.url);
            futures.push(tokio::spawn(async move {
                send_request(rpc.url.clone(), json_value_clone.clone())
            }));
        }
        while let Some(result) = futures.next().await {
            match result {
                // check is response transformed to serde_json have result field
                Ok(response) => {
                    let response_string = response.await.unwrap();
                    results.push(response_string.clone());
                    let response_json: Value = serde_json::from_str(&response_string).unwrap();
                    if response_json["result"].is_null() {
                        continue;
                    }
                    info!("Sent: return correct response: {}", response_string);
                    return Ok(Response::new(response_string.clone()));
                },
                Err(e) => {
                    let json_rpc_error = JsonRpcErrorResponse::from(ApplicationError::new(
                        ErrorCode::InternalServerError,
                        e.to_string(),
                    ));
                    error!("Error: {}", json_rpc_error.error.format_error().as_str());
                    results.push(json_rpc_error.to_json());
                    continue;
                },
            }
        }

        info!("None of the RPC nodes responded successfully: {} ", results[0].clone());
        Ok(Response::new(results[0].clone()))
    };

    //Return last Response
    return last_response;
}

pub async fn forward_rpc_request(rpc_list: Arc<Mutex<Vec<Rpc>>>, chain_id: usize,
                                 json_value: Value, algo: Algo,
) -> Result<Response<String>, hyper::Error> {

    let start_time = Instant::now();

    let rpc_list_copy = {
        let rpc_guard = rpc_list.lock().unwrap();
        // Here only the operation that needs exclusive access to the data is performed.
        // In this case, a copy of the data is being made.
        rpc_guard.clone()
    };

    // Filter the RPCs by chain ID if chain_id is not 0
    let filtered_rpc_list: Vec<&Rpc> = if chain_id != 0 {
        rpc_list_copy
            .iter()
            .filter(|rpc| rpc.chain_id == chain_id)
            .collect()
    } else {
        let json_response = JsonRpcErrorResponse::from(ApplicationError::new(
            ErrorCode::BadRequest,
            "chain_id path required (i.e. https://127.0.0.1:3000/10)".to_string(),
        ));
        // log error with cause and url received
        error!("Error: {}", json_response.error.format_error().as_str());
        return Ok(Response::new(json_response.to_json()));
    };

    if filtered_rpc_list.is_empty() {
        let json_response = JsonRpcErrorResponse::from(ApplicationError::new(
            ErrorCode::NotFound,
            "No RPC nodes found for the specified chain ID".to_string(),
        ));
        error!("Error: {}", json_response.error.format_error().as_str());
        return Ok(Response::new(json_response.to_json()));
    }



    let sorted_rpc_list = sort_rpc_list_by_algo(algo, filtered_rpc_list);
    info!("sorted_rpc_list: {:?}", sorted_rpc_list);

    // Loop through the sorted RPC list and make a request to each RPC until a successful response is received
    for rpc in sorted_rpc_list {
        debug!("Sending request {} to: {}", json_value, rpc.url);
        let intra_latency = start_time.elapsed().as_micros() as u64;
        let response = send_request(rpc.url.clone(), json_value.clone()).await;
        let total_latency = start_time.elapsed().as_micros() as u64;
        if response.is_ok() {
            {
                let mut rpc_guard = rpc_list.lock().unwrap();
                let index = rpc_guard
                    .iter()
                    .position(|r| r.eq(rpc))
                    .unwrap();
                // Here only the operation that needs exclusive access to the data is performed.
                // In this case, update the latencies and arrival timestamps for the chosen RPC.
                rpc_guard[index].intra_latencies.push(intra_latency);
                rpc_guard[index].srv_latencies.push(total_latency - intra_latency);
                rpc_guard[index].arrivals_ts.push(chrono::Utc::now().timestamp_millis() as u64);

                // debug all rpc_guard
                debug!("rpc_guard: {:?}", rpc_guard);
            };

            info!("Sent: Block Latency {} Intra Latency: {} Server Latency: {} RPC: {}",
                chrono::Utc::now().timestamp_millis() as u64 - rpc.last_block_ts,
                intra_latency,
                total_latency,
                rpc.url.split("/").collect::<Vec<&str>>()[2],
            );

            // info!("Block Latency: {} ms", chrono::Utc::now().timestamp_millis() as u64 - rpc.last_block_ts);
            // info!("Intra_latency: {} μs.", intra_latency);
            // info!("Srv_latency: {} μs.", total_latency - intra_latency);
            // debug!("Total_latency: {} μs.", total_latency);
            debug!("Response: {:?}", response);

            let response_string = response.unwrap();
            return Ok(Response::new(response_string));
        }
    }

    // If no successful response is received after iterating over the entire list, return an error
    let json_response = JsonRpcErrorResponse::from(ApplicationError::new(
        ErrorCode::InternalServerError,
        "No RPC nodes responded successfully".to_string(),
    ));
    error!("Error: {}", json_response.error.format_error().as_str());
    Ok(Response::new(json_response.to_json()))
}

pub async fn send_request(url: String, tx: Value) -> Result<String, hyper::Error> {

    let response = match CLIENT.post(url).json(&tx).send().await {
        Ok(response) => {
            if response.status().is_success() {
                response
            } else {
                let app_error = {
                    if response.status().is_client_error() {
                        ApplicationError::new(
                            ErrorCode::BadRequest,
                            "Bad request".to_string(),
                        )
                    } else if response.status().is_server_error() {
                        ApplicationError::new(
                            ErrorCode::InternalServerError,
                            "Internal server error".to_string(),
                        )
                    } else {
                        ApplicationError::new(
                            ErrorCode::UnknownError,
                            response.status().to_string(),
                        )
                    }
                };
                let error_response = JsonRpcErrorResponse::from(app_error);
                error!("Error: {}", error_response.error.format_error().as_str());
                return Ok(error_response.to_json());
            }
        },
        Err(error) => {
            let app_error = {
                if error.is_timeout() {
                    ApplicationError::new(
                        ErrorCode::RequestTimeout,
                        "Request to RPC node timed out".to_string(),
                    )
                } else if error.is_connect() {
                    ApplicationError::new(
                        ErrorCode::HandleConnectionError,
                        "RPC node network connection error".to_string(),
                    )
                } else {
                    ApplicationError::new(
                        ErrorCode::UnknownError,
                        error.to_string(),
                    )
                }
            };

            let error_response = JsonRpcErrorResponse::from(app_error);
            error!("Error: {}", error_response.error.format_error().as_str());
            return Ok(error_response.to_json());
        }
    };

    let response_text = response.text().await.unwrap();


    Ok(response_text)
}

pub async fn add_rpc(rpc_list: Arc<Mutex<Vec<Rpc>>>, add_rpc_request: AddRpcRequest,
                     stat_vec_size: usize) -> Result<Response<String>, hyper::Error> {
    let already_added=
        {
            let rpc_guard = rpc_list.lock().unwrap();
            rpc_guard.iter().any(|rpc| rpc.url == add_rpc_request.url)
        };
    if already_added {
        let response = JsonRpcResponse::from("Rpc already added".to_string());
        info!("{}: {}", response.result, add_rpc_request.url);
        return Ok(Response::new(response.to_json()));
    }
    let add_rpc_request_clone = add_rpc_request.clone();

    let rpc = Rpc::new(add_rpc_request.url,
                       add_rpc_request.ws_url,
                       add_rpc_request.chain_id,
                       RpcLocation::from_str(add_rpc_request.rpc_location.as_str()).unwrap(),
                       stat_vec_size).await;
    let rpc_clone =
        {
            let mut rpc_guard = rpc_list.lock().unwrap();
            let rpc_result = rpc.clone();
            rpc_guard.push(rpc);
            debug!("Rpc_guard after add_rpc : {:?}", rpc_guard);
            rpc_result
        };
    let len = {
        let rpc_guard = rpc_list.lock().unwrap();
        rpc_guard.len()
    };
    let rpc_list_clone = rpc_list.clone();
    let rpc_websocket =
        RpcWebSocket::new(rpc_clone.ws_url.clone());
    let mut rpc_websocket_clone = rpc_websocket.await.clone();
    tokio::task::spawn(async move {
        rpc_websocket_clone.start_rpc(rpc_list_clone, len - 1)
            .await;
    });

    let response = JsonRpcResponse::from("Rpc added successfully".to_string());
    info!("{}: {}", response.result, add_rpc_request_clone.url);
    Ok(Response::new(response.to_json()))
}

pub fn extract_chain_id(path: &str) -> usize {
    // Extract the chain ID from the request path
    let segments: Vec<&str> = path.split('/').collect();
    let chain_id = segments
        .last()
        .unwrap_or(&"0")
        .parse::<usize>()
        .unwrap_or(0);
    debug!("chain_id: {}", chain_id);
    chain_id
}

pub async fn incoming_to_value(
    request: Request<hyper::body::Incoming>,
) -> Result<Value, hyper::Error> {
    let tx = request.collect().await?.to_bytes().clone();
    let mut tx = from_utf8(&tx).unwrap().to_owned();

    let ret = match unsafe { from_str(&mut tx) } {
        Ok(ret) => ret,
        Err(_) => {
            let error_response = JsonRpcErrorResponse::from(ApplicationError::new(
                ErrorCode::BadRequest,
                "Invalid Json".to_string(),
            ));
            error!("Error: {}", error_response.error.format_error().as_str());
            return Ok(Value::String(error_response.to_json()));
        }
    };

    Ok(ret)
}