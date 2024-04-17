use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum Algo {
    MinLatency,
    RoundRobin,
    Broadcast,
}

impl FromStr for Algo {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "min_latency" => Ok(Algo::MinLatency),
            "round_robin" => Ok(Algo::RoundRobin),
            "broadcast" => Ok(Algo::Broadcast),
            _ => Err(()),
        }
    }
}

impl Default for Algo {
    fn default() -> Self {
        Algo::MinLatency
    }
}