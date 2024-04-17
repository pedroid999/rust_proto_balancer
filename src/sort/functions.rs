use crate::{
    rpc::types::{
        Rpc,
        RpcLocation,
        LimitedVecDeque,
    },
    sort::types::Algo,
};

use log::{info};

pub fn sort_rpc_list_by_algo(algo: Algo, filtered_rpc_list: Vec<&Rpc>) -> Vec<&Rpc> {
    match algo {
        Algo::MinLatency => {
            // Sort the RPC list by block number (descending), RpcLocation local preference and timestamp (ascending)
            min_latency_sort(filtered_rpc_list)
        },
        Algo::RoundRobin => {
            // Sort the RPC list by block number (descending), RpcLocation local preference and rpc_requests_per_minute (ascending)
            round_robin_sort(filtered_rpc_list)
        },
        // Add other Algo variants here
        _ => {
            // Default Sort the RPC list by block number (ascending) and timestamp (ascending)
            min_latency_sort(filtered_rpc_list)
        },
    }
}

pub fn min_latency_sort(mut filtered_rpc_list: Vec<&Rpc>) -> Vec<&Rpc> {
    // Sort the RPC list by block number (descending), RpcLocation::Local preference, and timestamp (ascending)
    filtered_rpc_list.sort_by(|a, b| {
        info!("url: {}, last_block: {}, current_ts: {}", a.url, a.last_block, a.current_ts);
        info!("url: {}, last_block: {}, current_ts: {}", b.url, b.last_block, b.current_ts);
        let location_order = compare_rpc_location(&a.rpc_location, &b.rpc_location);
        b.last_block
            .cmp(&a.last_block)
            .then_with(|| location_order)
            .then_with(|| a.current_ts.cmp(&b.current_ts))
    });
    filtered_rpc_list
}

pub fn rpc_requests_per_minute(arrivals_ts: &LimitedVecDeque) -> f64 {
    if arrivals_ts.deque.len() < 2 {
        return 0.0;
    }

    let first_arrival_ts = *arrivals_ts.deque.back().unwrap();
    let last_arrival_ts = *arrivals_ts.deque.front().unwrap();
    let difference_in_minutes = (last_arrival_ts - first_arrival_ts) as f64 / 60000.0;

    arrivals_ts.deque.len() as f64 / difference_in_minutes
}

pub fn round_robin_sort(mut filtered_rpc_list: Vec<&Rpc>) -> Vec<&Rpc> {
    // Sort the RPC list by block number (descending), RpcLocation::Internal preference, and rpc_requests_per_minute (ascending)
    filtered_rpc_list.sort_by(|a, b| {
        let location_order = compare_rpc_location(&a.rpc_location, &b.rpc_location);
        b.last_block
            .cmp(&a.last_block)
            .then_with(|| location_order)
            .then_with(|| {
                let a_arrivals_per_minute = rpc_requests_per_minute(&a.arrivals_ts);
                info!("url: {}, last_block: {}, a_arrivals_per_minute: {}", a.url, a.last_block, a_arrivals_per_minute);
                let b_arrivals_per_minute = rpc_requests_per_minute(&b.arrivals_ts);
                info!("url: {}, last_block: {}, b_arrivals_per_minute: {}", b.url, b.last_block, b_arrivals_per_minute);
                a_arrivals_per_minute.partial_cmp(&b_arrivals_per_minute).unwrap()
            })
    });
    filtered_rpc_list
}

pub fn compare_rpc_location(a: &RpcLocation, b: &RpcLocation) -> std::cmp::Ordering {
    match (a, b) {
        (RpcLocation::Local, RpcLocation::External) => std::cmp::Ordering::Less,
        (RpcLocation::External, RpcLocation::Local) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    }
}