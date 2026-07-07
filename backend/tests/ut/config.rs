use revx_bot::core::config::{dummy_config_path, expand_tilde, DummyConfig};
use serial_test::serial;

#[test]
fn test_expand_tilde() {
    let no_tilde = "/absolute/path/file.txt";
    assert_eq!(expand_tilde(no_tilde), no_tilde);

    if let Some(home) = dirs::home_dir() {
        let path = "~/test_file.txt";
        let expanded = expand_tilde(path);
        assert!(expanded.starts_with(&home.to_string_lossy().to_string()));
        assert!(expanded.ends_with("/test_file.txt"));
    }
}

#[test]
fn test_proxy_addr() {
    let cfg = DummyConfig {
        api_key: "".into(),
        private_key_path: "".into(),
        poll_interval_ms: 0,
        db_path: "".into(),
        port: 30091,
        base_url: None,
        symbols: vec![],
    };
    assert_eq!(cfg.proxy_addr(), "127.0.0.1:30091");
}

#[test]
#[serial]
fn test_dummy_config_path_resolution() {
    // Test environment override
    std::env::set_var("REVOLUTX_CONFIG_DIR", "/tmp/mock-dir");
    let path = dummy_config_path();
    assert_eq!(path.to_string_lossy(), "/tmp/mock-dir/dummy_config.json");

    std::env::remove_var("REVOLUTX_CONFIG_DIR");
}

#[test]
#[serial]
fn test_config_environmental_loading() {
    // 1. Success case
    let temp_dir = std::env::temp_dir().join("revx-test-cfg-env");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let cfg_path = temp_dir.join("dummy_config.json");
    let json = r#"{
        "api_key": "test-key",
        "private_key_path": "~/key.pem",
        "polling_interval_ms": 5000,
        "db_path": "~/test.db",
        "api_port": 8080,
        "symbols": []
    }"#;
    std::fs::write(&cfg_path, json).unwrap();

    std::env::set_var("REVOLUTX_CONFIG_DIR", temp_dir.to_str().unwrap());
    let cfg = DummyConfig::load().expect("should load config from env dir");
    assert_eq!(cfg.api_key, "test-key");

    // 2. Failure case (missing file)
    let missing_dir = temp_dir.join("missing");
    std::env::set_var("REVOLUTX_CONFIG_DIR", missing_dir.to_str().unwrap());
    let result = DummyConfig::load();
    assert!(result.is_err(), "should fail when dummy_config.json is missing");

    std::env::remove_var("REVOLUTX_CONFIG_DIR");
}

#[test]
#[serial]
fn test_stateless_config_load_success() {
    let temp_dir = std::env::temp_dir().join("revx-test-stateless-cfg-env");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let cfg_path = temp_dir.join("dummy_config.json");
    let json = r#"{
        "api_key": "st-key",
        "private_key_path": "~/st_key.pem",
        "polling_interval_ms": 500,
        "db_path": "~/st_test.db",
        "api_port": 9091,
        "symbols": [
            {
                "symbol": "BTC-USD",
                "buy_trigger_price": 50000.0,
                "sell_trigger_price": 60000.0,
                "revert_price": 55000.0,
                "trade_size_base": 0.01,
                "trade_size_quote": 0.0
            }
        ]
    }"#;
    std::fs::write(&cfg_path, json).unwrap();

    std::env::set_var("REVOLUTX_CONFIG_DIR", temp_dir.to_str().unwrap());
    let cfg = revx_bot::core::config::DummyConfig::load().expect("should load stateless config");
    assert_eq!(cfg.api_key, "st-key");
    assert_eq!(cfg.symbols.len(), 1);
    assert_eq!(cfg.symbols[0].symbol, "BTC-USD");

    std::env::remove_var("REVOLUTX_CONFIG_DIR");
}

#[test]
#[serial]
fn test_dummy_config_paths_fallback() {
    // Unset env var to hit fallback code path
    let old_var = std::env::var("REVOLUTX_CONFIG_DIR").ok();
    std::env::remove_var("REVOLUTX_CONFIG_DIR");

    let p1 = revx_bot::core::config::dummy_config_path();
    assert!(p1.to_string_lossy().contains("dummy_config.json"));

    if let Some(v) = old_var {
        std::env::set_var("REVOLUTX_CONFIG_DIR", v);
    }
}

#[test]
#[serial]
fn test_config_load_missing_files() {
    let temp_dir = std::env::temp_dir().join("revx-test-cfg-missing");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::env::set_var("REVOLUTX_CONFIG_DIR", temp_dir.to_str().unwrap());

    assert!(revx_bot::core::config::DummyConfig::load().is_err());

    std::env::remove_var("REVOLUTX_CONFIG_DIR");
}

#[test]
#[serial]
fn test_config_load_invalid_json() {
    let temp_dir = std::env::temp_dir().join("revx-test-cfg-invalid");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let cfg_path = temp_dir.join("dummy_config.json");
    std::fs::write(&cfg_path, "{ invalid").unwrap();

    std::env::set_var("REVOLUTX_CONFIG_DIR", temp_dir.to_str().unwrap());
    let res = revx_bot::core::config::DummyConfig::load();
    assert!(res.is_err());
    assert!(res.err().unwrap().to_string().contains("not valid JSON"));
    std::env::remove_var("REVOLUTX_CONFIG_DIR");
}

#[test]
fn test_default_tick_sizes() {
    let json = r#"{
        "symbol": "BTC-USD",
        "buy_trigger_price": 50000.0,
        "sell_trigger_price": 60000.0,
        "revert_price": 55000.0,
        "trade_size_base": 0.01,
        "trade_size_quote": 0.0
    }"#;
    let cfg: revx_bot::core::config::SymbolConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.tick_size, 0.0001);
}

#[test]
#[serial]
fn test_dummy_config_paths_no_home_fallback() {
    let old_cfg_dir = std::env::var("REVOLUTX_CONFIG_DIR").ok();

    std::env::remove_var("REVOLUTX_CONFIG_DIR");
    std::env::set_var("MOCK_NO_HOME", "1");

    let p1 = revx_bot::core::config::dummy_config_path();

    assert!(p1.to_string_lossy().ends_with("dummy_config.json"));
    assert!(!p1.to_string_lossy().contains(".config/revolut-x"));
    assert_eq!(revx_bot::core::config::expand_tilde("~/test"), "~/test");

    if let Some(v) = old_cfg_dir {
        std::env::set_var("REVOLUTX_CONFIG_DIR", v);
    }
    std::env::remove_var("MOCK_NO_HOME");
}
