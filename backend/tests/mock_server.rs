use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use rcgen::generate_simple_self_signed;

pub struct MockHttpsServer {
    pub uri: String,
    pub cert_pem: Vec<u8>,
}

impl MockHttpsServer {
    pub async fn start(app: Router) -> Self {
        rustls::crypto::aws_lc_rs::default_provider().install_default().ok();
        let subject_alt_names = vec!["127.0.0.1".to_string(), "localhost".to_string()];
        let cert = generate_simple_self_signed(subject_alt_names).unwrap();

        // In rcgen 0.14, CertifiedKey has `signing_key` field which is a `KeyPair`.
        let cert_pem = cert.cert.pem().into_bytes();
        let key_pem = cert.signing_key.serialize_pem().into_bytes();

        let config = RustlsConfig::from_pem(cert_pem.clone(), key_pem).await.unwrap();

        let handle = axum_server::Handle::<std::net::SocketAddr>::new();
        let handle_clone = handle.clone();

        tokio::spawn(async move {
            axum_server::bind_rustls("127.0.0.1:0".parse().unwrap(), config)
                .handle(handle_clone)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });

        // wait for it to bind
        let addr = loop {
            if let Some(addr) = handle.listening().await {
                break addr;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        };

        Self { uri: format!("https://{}", addr), cert_pem }
    }
}
