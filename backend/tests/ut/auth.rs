use ed25519_dalek::SigningKey;
use rand::{rngs::SysRng, TryRng};
use revx_bot::api::auth::{load_signing_key, sign_request};

#[test]
fn test_sign_request_basic() {
    let mut csprng = SysRng;
    let mut secret_bytes = [0u8; 32];
    csprng.try_fill_bytes(&mut secret_bytes).unwrap();
    let key = SigningKey::from_bytes(&secret_bytes);

    let ts = "1234567890";
    let method = "GET";
    let path = "/api/v1/test";
    let query = "a=1";
    let body = "";

    let sig = sign_request(&key, ts, method, path, query, body);
    assert!(!sig.is_empty());
}

#[test]
fn test_sign_request_full() {
    let mut csprng = SysRng;
    let mut secret_bytes = [0u8; 32];
    csprng.try_fill_bytes(&mut secret_bytes).unwrap();
    let key = SigningKey::from_bytes(&secret_bytes);

    let sig1 = sign_request(&key, "1000", "POST", "/api/v1/orders", "query=1", "{\"a\":1}");
    let sig2 = sign_request(&key, "1000", "POST", "/api/v1/orders", "query=1", "{\"a\":1}");
    assert_eq!(sig1, sig2);
}

#[test]
fn test_load_signing_key_invalid_path() {
    let result = load_signing_key("/non/existent/path/key.pem");
    assert!(result.is_err());
}

#[test]
fn test_load_signing_key_from_valid_pem() {
    let mut csprng = SysRng;
    let mut secret_bytes = [0u8; 32];
    csprng.try_fill_bytes(&mut secret_bytes).unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&secret_bytes);

    use pkcs8::EncodePrivateKey;
    let pem = key.to_pkcs8_pem(pkcs8::LineEnding::LF).unwrap();

    let temp_file = std::env::temp_dir().join("test_auth_key.pem");
    std::fs::write(&temp_file, pem.as_bytes()).unwrap();

    let loaded = load_signing_key(temp_file.to_str().unwrap()).unwrap();
    assert_eq!(loaded.to_bytes(), key.to_bytes());
}

#[test]
fn test_load_signing_key_invalid_pem() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("invalid.pem");
    std::fs::write(&path, "not a pem").unwrap();

    let res = load_signing_key(path.to_str().unwrap());
    assert!(res.is_err());
    assert!(res.err().unwrap().to_string().contains("cannot parse"));
}

#[test]
fn test_sign_request_special_chars() {
    let mut secret_bytes = [0u8; 32];
    secret_bytes[0] = 1;
    let key = SigningKey::from_bytes(&secret_bytes);

    let sig1 = sign_request(&key, "100", "GET", "/p", "s=BTC-USD,ETH-USD", "");
    let sig2 = sign_request(&key, "100", "GET", "/p", "s=BTC-USD,ETH-USD", "");
    assert_eq!(sig1, sig2);
}

#[test]
fn test_sign_request_slash_path() {
    let mut secret_bytes = [0u8; 32];
    secret_bytes[0] = 1;
    let key = SigningKey::from_bytes(&secret_bytes);

    let sig = sign_request(&key, "100", "GET", "/", "", "");
    assert!(!sig.is_empty());
}

#[test]
fn test_sign_request_large_body() {
    let mut secret_bytes = [0u8; 32];
    secret_bytes[0] = 1;
    let key = SigningKey::from_bytes(&secret_bytes);

    let body = "{\"data\": \"".to_string() + &"a".repeat(1000) + "\"}";
    let sig = sign_request(&key, "100", "POST", "/p", "", &body);
    assert!(!sig.is_empty());
}

#[test]
fn test_sign_request_methods() {
    let mut secret_bytes = [0u8; 32];
    secret_bytes[0] = 5;
    let key = SigningKey::from_bytes(&secret_bytes);

    let verbs = ["PUT", "DELETE", "PATCH"];
    for v in verbs {
        let sig1 = sign_request(&key, "100", v, "/orders", "", "");
        let sig2 = sign_request(&key, "100", v, "/orders", "", "");
        assert_eq!(sig1, sig2);
    }
}

#[test]
fn test_sign_request_trailing_slash() {
    let mut secret_bytes = [0u8; 32];
    secret_bytes[0] = 10;
    let key = SigningKey::from_bytes(&secret_bytes);

    let sig1 = sign_request(&key, "100", "GET", "/api/", "", "");
    let sig2 = sign_request(&key, "100", "GET", "/api/", "", "");
    assert_eq!(sig1, sig2);
}
