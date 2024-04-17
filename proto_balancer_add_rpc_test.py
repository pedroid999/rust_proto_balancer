import requests
import random

def add_rpc():
    payload = {
        "url": "https://radial-restless-county.optimism.quiknode.pro/076b9257f675bc800b8ac4844bb631ffb9623bb8/",
        "ws_url": "wss://radial-restless-county.optimism.quiknode.pro/076b9257f675bc800b8ac4844bb631ffb9623bb8/",
        "chain_id": 10,
        "rpc_location": "External",
    }
    response = requests.post("http://127.0.0.1:3003/10", json=payload)
    result = response.json()
    return result

def main():
    for i in range(1):
        add_rpc_result = add_rpc()
        print(f"Add rpc result: {add_rpc_result}")

if __name__ == "__main__":
    main()
