use axum_server::tls_openssl::OpenSSLConfig;
use openssl::pkey::PKey;
use openssl::ssl::{SslAcceptor, SslMethod, SslVersion};
use openssl::x509::X509;
use std::io;
use std::path::Path;

pub fn server_config_from_pem(cert_pem: &[u8], key_pem: &[u8]) -> io::Result<OpenSSLConfig> {
    let mut certificates = X509::stack_from_pem(cert_pem)
        .map_err(io::Error::other)?
        .into_iter();
    let certificate = certificates.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "TLS certificate chain is empty")
    })?;
    let private_key = PKey::private_key_from_pem(key_pem).map_err(io::Error::other)?;
    let mut acceptor =
        SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server()).map_err(io::Error::other)?;
    acceptor
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .map_err(io::Error::other)?;
    acceptor
        .set_certificate(&certificate)
        .map_err(io::Error::other)?;
    acceptor
        .set_private_key(&private_key)
        .map_err(io::Error::other)?;
    for certificate in certificates {
        acceptor
            .add_extra_chain_cert(certificate)
            .map_err(io::Error::other)?;
    }
    OpenSSLConfig::try_from(acceptor).map_err(io::Error::other)
}

pub async fn server_config_from_pem_file(
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
) -> io::Result<OpenSSLConfig> {
    let cert_pem = tokio::fs::read(cert_path).await?;
    let key_pem = tokio::fs::read(key_path).await?;
    server_config_from_pem(&cert_pem, &key_pem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_server::tls_openssl::OpenSSLAcceptor;

    #[test]
    fn preserves_certificate_chain() {
        let leaf = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let intermediate = rcgen::generate_simple_self_signed(vec!["issuer".into()]).unwrap();
        let chain = format!("{}{}", leaf.cert.pem(), intermediate.cert.pem());
        let config = server_config_from_pem(
            chain.as_bytes(),
            leaf.signing_key.serialize_pem().as_bytes(),
        )
        .unwrap();
        assert_eq!(config.get_inner().context().extra_chain_certs().len(), 1);
    }

    #[test]
    fn rejects_empty_invalid_and_mismatched_credentials() {
        let first = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let second = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert = first.cert.pem();
        let key = first.signing_key.serialize_pem();
        assert!(server_config_from_pem(b"", key.as_bytes()).is_err());
        assert!(server_config_from_pem(b"invalid", key.as_bytes()).is_err());
        assert!(server_config_from_pem(cert.as_bytes(), b"invalid").is_err());
        assert!(server_config_from_pem(
            cert.as_bytes(),
            second.signing_key.serialize_pem().as_bytes(),
        )
        .is_err());
    }

    #[tokio::test]
    async fn https_client_verifies_certificates_and_supports_tls12() {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.cert.pem();
        let config = server_config_from_pem(
            cert_pem.as_bytes(),
            cert.signing_key.serialize_pem().as_bytes(),
        )
        .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let handle = axum_server::Handle::new();
        let app = axum::Router::new().route("/", axum::routing::get(|| async { "tls-ok" }));
        let server = axum_server::from_tcp(listener)
            .unwrap()
            .acceptor(OpenSSLAcceptor::new(config))
            .handle(handle.clone())
            .serve(app.into_make_service());
        let task = tokio::spawn(server);
        handle.listening().await.unwrap();
        let url = format!("https://localhost:{}/", address.port());
        let root = reqwest::Certificate::from_pem(cert_pem.as_bytes()).unwrap();
        for tls12_only in [false, true] {
            let mut client = reqwest::Client::builder()
                .add_root_certificate(root.clone())
                .timeout(std::time::Duration::from_secs(5));
            if tls12_only {
                client = client.max_tls_version(reqwest::tls::Version::TLS_1_2);
            }
            let response = client.build().unwrap().get(&url).send().await.unwrap();
            assert_eq!(response.text().await.unwrap(), "tls-ok");
        }
        let untrusted = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        assert!(untrusted.get(&url).send().await.is_err());
        handle.shutdown();
        task.await.unwrap().unwrap();
    }
}
