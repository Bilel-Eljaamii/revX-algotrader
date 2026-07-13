import json
import time
import base64
import requests
import os
from cryptography.hazmat.primitives import serialization

def load_config():
    home = os.path.expanduser("~")
    path = os.path.join(home, ".config", "revolut-x", "config.json")
    print(f"🐞 [DEBUG] Loading config from: {path}")

    if not os.path.exists(path):
        print(f"❌ [ERROR] Config file not found at: {path}")
        raise FileNotFoundError(f"Config file not found: {path}")

    try:
        with open(path, "r") as f:
            cfg = json.load(f)
        print(f"✅ [INFO] Config loaded successfully. Keys found: {list(cfg.keys())}")
        return cfg
    except json.JSONDecodeError as e:
        print(f"❌ [ERROR] Failed to parse config JSON: {e}")
        raise

def load_private_key(path):
    path = os.path.expanduser(path)
    print(f"🐞 [DEBUG] Loading private key from: {path}")

    if not os.path.exists(path):
        print(f"❌ [ERROR] Private key file not found at: {path}")
        raise FileNotFoundError(f"Private key not found: {path}")

    try:
        with open(path, "rb") as f:
            pem_data = f.read()
        print(f"✅ [INFO] Private key file read successfully. Size: {len(pem_data)} bytes.")

        # Trying to load without a password. If it requires one, it will throw an exception here.
        key = serialization.load_pem_private_key(pem_data, password=None)
        print(f"✅ [INFO] Private key parsed successfully.")
        return key
    except Exception as e:
        print(f"❌ [ERROR] Failed to load private key. Is it password protected? Error: {e}")
        raise

def test_api():
    try:
        cfg = load_config()
    except Exception as e:
        print("🛑 [ABORT] Stopping due to config error.")
        return

    try:
        key = load_private_key(cfg["private_key_path"])
    except Exception as e:
        print("🛑 [ABORT] Stopping due to key loading error.")
        return

    api_key = cfg["api_key"]
    base_url = cfg.get("base_url", "https://revx.revolut.com")

    # 1. Query with USDC-USD
    path = "/api/1.0/orders/active"
    query = "symbols=USDC-USD"
    method = "GET"

    # Timestamp calculation
    raw_timestamp = time.time()
    timestamp = str(int((raw_timestamp - 1.5) * 1000))

    print(f"🐞 [DEBUG] Request Details:")
    print(f"    Method: {method}")
    print(f"    Path: {path}")
    print(f"    Query: {query}")
    print(f"    Timestamp (ms): {timestamp}")

    message = f"{timestamp}{method}{path}{query}".encode()
    print(f"🐞 [DEBUG] Signing Message (raw string): {message.decode()}")

    try:
        signature_bytes = key.sign(message)
        signature = base64.b64encode(signature_bytes).decode()
        print(f"✅ [INFO] Signature generated successfully.")
        print(f"🐞 [DEBUG] Generated Signature: {signature[:20]}... (truncated for safety, check exact value if debugging auth)")
    except Exception as e:
        print(f"❌ [ERROR] Signature generation failed: {e}")
        return

    url = f"{base_url}{path}?{query}"
    headers = {
        "X-Revx-API-Key": api_key,
        "X-Revx-Timestamp": timestamp,
        "X-Revx-Signature": signature,
        "Content-Type": "application/json"
    }

    print(f"🐞 [DEBUG] Request URL: {url}")
    # Redacting sensitive info so you don't leak them to logs by mistake. If you need the raw value, change '***REDACTED***' to 'v'.
    print(f"🐞 [DEBUG] Request Headers: {json.dumps({k: v if k not in ['X-Revx-API-Key', 'X-Revx-Signature'] else '***REDACTED***' for k, v in headers.items()}, indent=2)}")

    try:
        resp = requests.get(url, headers=headers)
        print(f"ℹ️ [INFO] Response Status Code: {resp.status_code}")
        print(f"🐞 [DEBUG] Response Headers: {dict(resp.headers)}")

        # Check response body
        try:
            if resp.status_code == 200:
                data = resp.json()
                print(f"✅ [INFO] JSON response parsed successfully.")
                print(f"✅ [INFO] Data length: {len(data['data'])}")
                print(f"🐞 [DEBUG] Response Data (truncated first 500 chars): {json.dumps(data, indent=2)[:500]}...")
            else:
                print(f"⚠️ [WARN] Request returned non-200 status code.")
                print(f"🐞 [DEBUG] Raw response body: {resp.text}")

                # Try to parse error message even if status != 200
                try:
                    err_json = resp.json()
                    print(f"🐞 [DEBUG] JSON Error Body: {json.dumps(err_json, indent=2)}")
                except:
                    pass
        except json.JSONDecodeError as e:
            print(f"❌ [ERROR] Failed to decode JSON response. Error: {e}")
            print(f"🐞 [DEBUG] Raw response body (first 500 chars): {resp.text[:500]}")
        except Exception as e:
            print(f"❌ [ERROR] Unexpected error handling response: {e}")

    except requests.exceptions.RequestException as e:
        print(f"❌ [ERROR] Network request failed. (Check base_url or internet connection): {e}")

if __name__ == "__main__":
    print(f"🔍 [START] Starting API Connection Test...")
    test_api()
    print(f"🔍 [END] Test finished.")
