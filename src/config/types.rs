use std::fs;
use std::net::SocketAddr;
use std::str::FromStr;
use clap::{
    Command,
    Arg,
};
use log::info;
use crate::{
    rpc::types::{
        Rpc,
        RpcLocation,
    },
    sort::types::Algo,
};


#[derive(Debug, Clone)]
pub struct Settings {
    pub rpc_list: Vec<Rpc>,
    pub address: SocketAddr,
    pub log_level: String,
    pub stats_vec_size: usize,
    pub algo: Algo,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rpc_list: Vec::new(),
            address: SocketAddr::from(([127, 0, 0, 1], 3000)),
            log_level: String::from("info"),
            stats_vec_size: 1000,
            algo: Algo::MinLatency,
        }
    }
}

impl Settings {
    pub async fn new(matches: Command) -> Settings {
        let matches = matches.get_matches();

        // Try to open the file at the path specified in the args
        let path = matches.get_one::<String>("config").unwrap();
        let file: Option<String> = match fs::read_to_string(path) {
            Ok(file) => Some(file),
            Err(_) => panic!("\x1b[31mErr:\x1b[0m Error opening config file at {}", path),
        };

        if let Some(file) = file {
            info!("\x1b[35mInfo:\x1b[0m Using config file at {}", path);
            return Settings::create_from_file(file).await;
        }
        else{
            panic!("\x1b[31mErr:\x1b[0m rpc_config.toml file does not exist");
        }
    }

    async fn create_from_file(conf_file: String) -> Settings {
        let parsed_toml = conf_file.parse::<toml::value::Value>().expect("Error parsing TOML");

        let table_names: Vec<&String> = parsed_toml.as_table().unwrap().keys().collect::<Vec<_>>();

        let proto_balancer_table = parsed_toml
            .get("proto_balancer")
            .expect("\x1b[31mErr:\x1b[0m Missing proto_balancer table!")
            .as_table()
            .expect("\x1b[31mErr:\x1b[0m Could not parse proto_balancer table!");
        let address = proto_balancer_table
            .get("address")
            .expect("\x1b[31mErr:\x1b[0m Missing address!")
            .as_str()
            .expect("\x1b[31mErr:\x1b[0m Could not parse address as str!");

        // Build the SocketAddr
        let port = 3000;
        // Replace `localhost` if it exists
        let address = address.replace("localhost", "127.0.0.1");
        // If the address contains `:` don't concatenate the port and just pass the address
        let address = if address.contains(':') {
            address.to_string()
        } else {
            format!("{}:{}", address, port)
        };
        let address = address
            .parse::<SocketAddr>()
            .expect("\x1b[31mErr:\x1b[0m Could not address to SocketAddr!");

        let log_level = proto_balancer_table
            .get("log_level")
            .expect("\x1b[31mErr:\x1b[0m Missing log_level!")
            .as_str()
            .expect("\x1b[31mErr:\x1b[0m Could not parse log_level as str!");

        let stats_vec_size = proto_balancer_table
            .get("stats_vec_size")
            .expect("\x1b[31mErr:\x1b[0m Missing stats_vec_size!")
            .as_integer()
            .expect("\x1b[31mErr:\x1b[0m Could not parse stats_vec_size as integer!") as usize;

        let algo = Algo::from_str(proto_balancer_table
            .get("algo")
            .and_then(|v| v.as_str())
            .unwrap_or("min_latency"))
            .unwrap_or(Algo::MinLatency);


        // Parse all the other tables as RPCs and put them in a Vec<Rpc>
        let mut rpc_list: Vec<Rpc> = Vec::new();
        for table_name in table_names {
            if table_name != "proto_balancer" {
                let rpc_table = parsed_toml.get(table_name).unwrap().as_table().unwrap();

                let url = String::from(rpc_table
                    .get("url")
                    .expect("\x1b[31mErr:\x1b[0m Missing url from an RPC!")
                    .as_str()
                    .expect("\x1b[31mErr:\x1b[0m Could not parse url as str!"));

                let ws_url = String::from(rpc_table
                    .get("ws_url")
                    .expect("\x1b[31mErr:\x1b[0m Missing ws_url from an RPC!")
                    .as_str()
                    .expect("\x1b[31mErr:\x1b[0m Could not parse ws_url as str!"));

                let chain_id = rpc_table
                    .get("chain_id")
                    .expect("\x1b[31mErr:\x1b[0m Missing chain_id from an RPC!")
                    .as_integer()
                    .expect("\x1b[31mErr:\x1b[0m Could not parse chain_id as integer!")
                    as usize;

                let rpc_location = match rpc_table
                    .get("rpc_location")
                    .expect("\x1b[31mErr:\x1b[0m Missing rpc_location from an RPC!")
                    .as_str()
                    .expect("\x1b[31mErr:\x1b[0m Could not parse rpc_location as String!")
                    .to_lowercase()
                    .as_str()
                {
                    "local" => RpcLocation::Local,
                    "external" => RpcLocation::External,
                    _ => panic!("\x1b[31mErr:\x1b[0m Invalid rpc_location!"),
                };

                let rpc = Rpc::new(url, ws_url, chain_id, rpc_location, stats_vec_size).await;
                rpc_list.push(rpc);
            }
        }

        Settings {
            rpc_list,
            address,
            log_level: log_level.to_string(),
            stats_vec_size,
            algo,
        }
    }

    pub fn create_match() -> Command {
        Command::new("proto_balancer")
            .version("0.1.0")
            .author("pedroid999 <pedrete999@gmail.com> and contributors")
            .about("load balancer prototype.")
            .arg(Arg::new("config")
                .long("config")
                .short('c')
                .num_args(1..)
                .default_value("rpc_config.toml")
                .help("TOML config file for load balancer prototype."))
    }
}