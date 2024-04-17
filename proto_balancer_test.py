import requests
import time
import threading

def get_last_block():
    payload = {
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1,
        }
    response = requests.post("http://127.0.0.1:3003/10", json=payload)
    result = response.json()
    return result

def get_block_ts(block):
    block_reference = 107168702
    ts_reference = 1689936181
    ts = ts_reference + (block - block_reference) * 2
    return ts

def main():
    while True:
        start = time.time()
        last_block = get_last_block()
        end = time.time()
        print(f"Time to get block: {end - start}")
        print(f"Block Number json: {last_block}")
        if last_block.get('error'):
            print(f"Error: {last_block.get('error')}")
            continue
        block_number = int(last_block.get('result', '').lstrip('0x'), 16)
        print(f"Block Number: {block_number}")
        print(f"Arrival minus expected: {time.time() - get_block_ts(block_number)}")
        # print(f"Arrival block timestamp: {time.time()}")
        # print(f"Expected block Timestamp: {get_block_ts(block_number)}")

        time.sleep(2)  # Pause for 100 milliseconds

if __name__ == "__main__":
    threads = []

    for i in range(1):
        t = threading.Thread(target=main)
        t.start()
        threads.append(t)

    for t in threads:
        t.join()
