import json
import os
import time
import base64
import requests
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519

def config_path():
    env_dir = os.environ.get("REVOLUTX_CONFIG_DIR")
    if env_dir:
        return os.path.join(env_dir, "config.json")
    home = os.path.expanduser("~")
    return os.path.join(home, ".config", "revolut-x", "config.json")

def load_config():
    path = config_path()
    with open(path, "r") as f:
        return json.load(f)

def load_private_key(path):
    path = os.path.expanduser(path)
    with open(path, "rb") as f:
        pem_data = f.read()
    return serialization.load_pem_private_key(pem_data, password=None)

def get_active_orders(cfg, private_key, symbol=None):
    api_key = cfg["api_key"]
    base_url = cfg.get("base_url", "https://revx.revolut.com")
    path = "/api/1.0/orders/active"
    
    query = f"symbols={symbol}" if symbol else ""
    method = "GET"
    
    # Use current time - 1.5s as the bot does
    timestamp = str(int((time.time() - 1.5) * 1000))
    
    message = f"{timestamp}{method}{path}{query}".encode()
    signature_bytes = private_key.sign(message)
    signature = base64.b64encode(signature_bytes).decode()
    
    url = f"{base_url}{path}"
    if query:
        url += f"?{query}"
        
    headers = {
        "X-Revx-API-Key": api_key,
        "X-Revx-Timestamp": timestamp,
        "X-Revx-Signature": signature,
        "Content-Type": "application/json"
    }
    
    resp = requests.get(url, headers=headers)
    if resp.status_code != 200:
        print(f"Error: {resp.status_code} - {resp.text}")
        return None
    
    return resp.json()

if __name__ == "__main__":
    try:
        cfg = load_config()
        key = load_private_key(cfg["private_key_path"])
        
        print(f"Loaded config from {config_path()}")
        
        print("\n--- Checking Active Orders (API) ---")
        data = get_active_orders(cfg, key)
        if data:
            print(f"Found {len(data['data'])} total active orders on exchange:")
            for o in data['data']:
                print(f"  ID: {o['id']} | Symbol: {o['symbol']} | Side: {o['side']} | Status: {o['status']} | Price: {o.get('limit_price', 'N/A')}")
        
    except Exception as e:
        print(f"Failed: {e}")
