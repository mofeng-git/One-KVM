use openssl::stack::Stack;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::verify::{X509CheckFlags, X509VerifyParam};
use openssl::x509::{X509PurposeId, X509StoreContext, X509};
use rustls_pki_types::{CertificateDer, ServerName};

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default)]
pub struct RootCertStore {
    certificates: Vec<X509>,
}

impl RootCertStore {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn add(&mut self, certificate: CertificateDer<'_>) -> Result<()> {
        self.certificates
            .push(X509::from_der(certificate.as_ref()).map_err(openssl_error)?);
        Ok(())
    }
}

pub(crate) struct CertificateVerifier {
    roots: RootCertStore,
}

impl CertificateVerifier {
    pub(crate) fn new(roots: RootCertStore) -> Self {
        Self { roots }
    }

    pub(crate) fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
    ) -> Result<()> {
        self.verify(end_entity, intermediates, X509PurposeId::SSL_CLIENT, None)
    }

    pub(crate) fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &str,
    ) -> Result<()> {
        ServerName::try_from(server_name).map_err(|error| Error::Other(error.to_string()))?;
        self.verify(
            end_entity,
            intermediates,
            X509PurposeId::SSL_SERVER,
            Some(server_name),
        )
    }

    fn verify(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        purpose: X509PurposeId,
        server_name: Option<&str>,
    ) -> Result<()> {
        let leaf = X509::from_der(end_entity.as_ref()).map_err(openssl_error)?;
        let mut chain = Stack::new().map_err(openssl_error)?;
        for certificate in intermediates {
            chain
                .push(X509::from_der(certificate.as_ref()).map_err(openssl_error)?)
                .map_err(openssl_error)?;
        }
        let mut parameters = X509VerifyParam::new().map_err(openssl_error)?;
        parameters.set_auth_level(2);
        parameters.set_purpose(purpose).map_err(openssl_error)?;
        parameters.set_hostflags(
            X509CheckFlags::NO_PARTIAL_WILDCARDS | X509CheckFlags::NEVER_CHECK_SUBJECT,
        );
        if let Some(server_name) = server_name {
            if let Ok(address) = server_name.parse() {
                parameters.set_ip(address).map_err(openssl_error)?;
            } else {
                parameters.set_host(server_name).map_err(openssl_error)?;
            }
        }
        let mut store = X509StoreBuilder::new().map_err(openssl_error)?;
        store.set_param(&parameters).map_err(openssl_error)?;
        for certificate in &self.roots.certificates {
            store.add_cert(certificate.clone()).map_err(openssl_error)?;
        }
        let store = store.build();
        let mut context = X509StoreContext::new().map_err(openssl_error)?;
        context
            .init(&store, &leaf, &chain, |context| {
                if context.verify_cert()? {
                    Ok(None)
                } else {
                    Ok(Some(context.error().to_string()))
                }
            })
            .map_err(openssl_error)?
            .map_or(Ok(()), |error| Err(Error::Other(error)))
    }
}

fn openssl_error(error: openssl::error::ErrorStack) -> Error {
    Error::Other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};

    fn signed_leaf(
        parameters: CertificateParams,
    ) -> (CertificateVerifier, CertificateDer<'static>) {
        let mut issuer_parameters = CertificateParams::new(Vec::<String>::new()).unwrap();
        issuer_parameters
            .distinguished_name
            .push(rcgen::DnType::CommonName, "test root");
        issuer_parameters.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let issuer_key = KeyPair::generate().unwrap();
        let issuer = issuer_parameters.self_signed(&issuer_key).unwrap();
        let leaf_key = KeyPair::generate().unwrap();
        let leaf = parameters
            .signed_by(&leaf_key, &issuer, &issuer_key)
            .unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(issuer.der().clone()).unwrap();
        (CertificateVerifier::new(roots), leaf.der().clone())
    }

    #[test]
    fn trusted_server_matches_dns_and_ip_subject_alternative_names() {
        let parameters =
            CertificateParams::new(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
        let (verifier, leaf) = signed_leaf(parameters);
        verifier
            .verify_server_cert(&leaf, &[], "localhost")
            .unwrap();
        assert!(verifier.verify_server_cert(&leaf, &[], "127.0.0.1").is_ok());
        assert!(verifier
            .verify_server_cert(&leaf, &[], "wrong.local")
            .is_err());
        assert!(verifier
            .verify_server_cert(&leaf, &[], "127.0.0.2")
            .is_err());
        assert!(verifier.verify_server_cert(&leaf, &[], "").is_err());
    }

    #[test]
    fn rejects_untrusted_and_tampered_certificates() {
        let (verifier, leaf) =
            signed_leaf(CertificateParams::new(vec!["localhost".into()]).unwrap());
        let untrusted = CertificateVerifier::new(RootCertStore::empty());
        assert!(untrusted
            .verify_server_cert(&leaf, &[], "localhost")
            .is_err());
        let mut tampered = leaf.as_ref().to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(verifier
            .verify_server_cert(&CertificateDer::from(tampered), &[], "localhost")
            .is_err());
        assert!(verifier
            .verify_server_cert(&CertificateDer::from(vec![0]), &[], "localhost")
            .is_err());
    }

    #[test]
    fn rejects_expired_certificate_and_wrong_extended_key_usage() {
        let mut expired = CertificateParams::new(vec!["localhost".into()]).unwrap();
        expired.not_before = rcgen::date_time_ymd(2019, 1, 1);
        expired.not_after = rcgen::date_time_ymd(2020, 1, 1);
        let (verifier, leaf) = signed_leaf(expired);
        assert!(verifier
            .verify_server_cert(&leaf, &[], "localhost")
            .is_err());
        let mut server_only = CertificateParams::new(vec!["localhost".into()]).unwrap();
        server_only.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let (verifier, leaf) = signed_leaf(server_only);
        verifier
            .verify_server_cert(&leaf, &[], "localhost")
            .unwrap();
        assert!(verifier.verify_client_cert(&leaf, &[]).is_err());
        let mut client_only = CertificateParams::new(vec!["localhost".into()]).unwrap();
        client_only.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let (verifier, leaf) = signed_leaf(client_only);
        assert!(verifier.verify_client_cert(&leaf, &[]).is_ok());
        assert!(verifier
            .verify_server_cert(&leaf, &[], "localhost")
            .is_err());
    }

    #[test]
    fn does_not_fall_back_to_common_name_or_partial_wildcard() {
        let mut parameters = CertificateParams::new(Vec::<String>::new()).unwrap();
        parameters
            .distinguished_name
            .push(rcgen::DnType::CommonName, "localhost");
        let (verifier, leaf) = signed_leaf(parameters);
        assert!(verifier
            .verify_server_cert(&leaf, &[], "localhost")
            .is_err());
        let (verifier, leaf) =
            signed_leaf(CertificateParams::new(vec!["local*.example".into()]).unwrap());
        assert!(verifier
            .verify_server_cert(&leaf, &[], "localhost.example")
            .is_err());
    }

    #[test]
    fn rejects_weak_rsa_keys_and_sha1_leaf_signatures() {
        use openssl::asn1::Asn1Time;
        use openssl::hash::MessageDigest;
        use openssl::pkey::PKey;
        use openssl::rsa::Rsa;
        use openssl::x509::extension::{BasicConstraints, SubjectAlternativeName};
        use openssl::x509::X509NameBuilder;

        let issuer_key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let mut issuer_name = X509NameBuilder::new().unwrap();
        issuer_name
            .append_entry_by_text("CN", "strong root")
            .unwrap();
        let issuer_name = issuer_name.build();
        let mut issuer = X509::builder().unwrap();
        issuer.set_version(2).unwrap();
        issuer.set_subject_name(&issuer_name).unwrap();
        issuer.set_issuer_name(&issuer_name).unwrap();
        issuer.set_pubkey(&issuer_key).unwrap();
        issuer
            .set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        issuer
            .set_not_after(&Asn1Time::days_from_now(1).unwrap())
            .unwrap();
        issuer
            .append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        issuer.sign(&issuer_key, MessageDigest::sha256()).unwrap();
        let issuer = issuer.build();
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(issuer.to_der().unwrap()))
            .unwrap();
        let verifier = CertificateVerifier::new(roots);
        for (bits, digest, expected) in [
            (2048, MessageDigest::sha256(), true),
            (1024, MessageDigest::sha256(), false),
            (2048, MessageDigest::sha1(), false),
        ] {
            let leaf_key = PKey::from_rsa(Rsa::generate(bits).unwrap()).unwrap();
            let mut leaf_name = X509NameBuilder::new().unwrap();
            leaf_name.append_entry_by_text("CN", "localhost").unwrap();
            let leaf_name = leaf_name.build();
            let mut leaf = X509::builder().unwrap();
            leaf.set_version(2).unwrap();
            leaf.set_subject_name(&leaf_name).unwrap();
            leaf.set_issuer_name(issuer.subject_name()).unwrap();
            leaf.set_pubkey(&leaf_key).unwrap();
            leaf.set_not_before(&Asn1Time::days_from_now(0).unwrap())
                .unwrap();
            leaf.set_not_after(&Asn1Time::days_from_now(1).unwrap())
                .unwrap();
            let names = SubjectAlternativeName::new()
                .dns("localhost")
                .build(&leaf.x509v3_context(Some(&issuer), None))
                .unwrap();
            leaf.append_extension(names).unwrap();
            leaf.sign(&issuer_key, digest).unwrap();
            let leaf = CertificateDer::from(leaf.build().to_der().unwrap());
            assert_eq!(
                verifier.verify_server_cert(&leaf, &[], "localhost").is_ok(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn dtls_handshake_preserves_mutual_certificate_authentication() {
        use crate::config::{ClientAuthType, Config};
        use crate::conn::DTLSConn;
        use crate::crypto::Certificate;
        use std::sync::Arc;
        use std::time::Duration;

        let client_certificate =
            Certificate::generate_self_signed(vec!["localhost".into()]).unwrap();
        let server_certificate =
            Certificate::generate_self_signed(vec!["localhost".into()]).unwrap();
        let mut client_roots = RootCertStore::empty();
        client_roots
            .add(server_certificate.certificate[0].clone())
            .unwrap();
        let mut server_roots = RootCertStore::empty();
        server_roots
            .add(client_certificate.certificate[0].clone())
            .unwrap();
        let client_socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let server_socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        client_socket
            .connect(server_socket.local_addr().unwrap())
            .await
            .unwrap();
        server_socket
            .connect(client_socket.local_addr().unwrap())
            .await
            .unwrap();
        let client_config = Config {
            certificates: vec![client_certificate],
            roots_cas: client_roots,
            server_name: "localhost".into(),
            ..Default::default()
        };
        let server_config = Config {
            certificates: vec![server_certificate],
            client_cas: server_roots,
            client_auth: ClientAuthType::RequireAndVerifyClientCert,
            ..Default::default()
        };
        let (client, server) = tokio::time::timeout(Duration::from_secs(10), async {
            tokio::try_join!(
                DTLSConn::new(client_socket, client_config, true, None),
                DTLSConn::new(server_socket, server_config, false, None),
            )
        })
        .await
        .unwrap()
        .unwrap();
        client
            .write(b"authenticated", Some(Duration::from_secs(5)))
            .await
            .unwrap();
        let mut buffer = [0; 64];
        let count = server
            .read(&mut buffer, Some(Duration::from_secs(5)))
            .await
            .unwrap();
        assert_eq!(&buffer[..count], b"authenticated");
        client.close().await.unwrap();
        server.close().await.unwrap();
    }

    #[test]
    fn verifies_intermediate_chain_without_implicitly_trusting_it() {
        let mut root_parameters = CertificateParams::new(Vec::<String>::new()).unwrap();
        root_parameters
            .distinguished_name
            .push(rcgen::DnType::CommonName, "test root");
        root_parameters.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let root_key = KeyPair::generate().unwrap();
        let root = root_parameters.self_signed(&root_key).unwrap();
        let mut intermediate_parameters = CertificateParams::new(Vec::<String>::new()).unwrap();
        intermediate_parameters
            .distinguished_name
            .push(rcgen::DnType::CommonName, "test intermediate");
        intermediate_parameters.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        let intermediate_key = KeyPair::generate().unwrap();
        let intermediate = intermediate_parameters
            .signed_by(&intermediate_key, &root, &root_key)
            .unwrap();
        let leaf_key = KeyPair::generate().unwrap();
        let leaf = CertificateParams::new(vec!["localhost".into()])
            .unwrap()
            .signed_by(&leaf_key, &intermediate, &intermediate_key)
            .unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(root.der().clone()).unwrap();
        let verifier = CertificateVerifier::new(roots);
        assert!(verifier
            .verify_server_cert(leaf.der(), &[intermediate.der().clone()], "localhost")
            .is_ok());
        assert!(verifier
            .verify_server_cert(leaf.der(), &[], "localhost")
            .is_err());
    }
}
