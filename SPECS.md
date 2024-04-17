# 🦀 Rust 🦀 Load Balancer Prototype Specifications

## V0 TODO

### Config
- Persist config.

### Logger
- The logger should not use the library name but a global process name (`load_balancer_{url}`).

### Stats Rpc Vectors
- On demand, we can generate statistics such as average, median, p50, p99, and others.
- **Path /get_stats**: Return json with basic stats per rpc (i.e., `{"average_latency": 0.5, "median_latency": 0.3, "p50": 0.2, "p99": 0.1, req_min: 1000}`).

### Algorithms
- **Add preference for Internal RpcLocation**: When the block number is equal, we should add a preference for the internal rpc location.
- **Round Robin Algorithm**: We could use an additional path to `chainId`, like this: `/chain_id/algo` (i.e., `/10/round_robin`).  (no additional path to not increase other apps complexity). Podriamos probar a hacer un nuevo order que sea en vez de por timestamp, por request/min y el que menos tenga va primero (siempre usando los de maximo bloque)
- **Broadcasting Algorithm**: Send the request to all nodes (the top 5 or something like that) and collect the responses in a vector (to reduce latency when you want to send transactions to the network).
- **Sort Algorithm Manager**: Transform the `rpc_list` sort into a function to also perform a match on the sort configuration and decide whether to do it standard sort, round-robin, or send spam mode to all, so we leave it prepared for the future to easily add things.

## V1 TODO

### Signer
### Cache
### Alarm System
### Backend Bundles
- **Calls**: i.e., calls vs sendtxs.

