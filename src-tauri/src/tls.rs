use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose,
    IsCa, Issuer, KeyPair, KeyUsagePurpose, SanType, PKCS_ECDSA_P256_SHA256,
};
use rustls_pki_types::CertificateDer;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::IpAddr,
    path::Path,
};
use time::{Duration, OffsetDateTime};

pub struct TlsMaterial {
    pub ca_der: Vec<u8>,
    pub server_certificate_der: Vec<u8>,
    pub server_key_der: Vec<u8>,
}

pub fn create(directory: &Path, server_ip: IpAddr) -> Result<TlsMaterial, String> {
    if server_ip.is_unspecified() || server_ip.is_loopback() {
        return Err("O certificado HTTPS precisa de um endereço acessível na rede local".into());
    }

    let (ca_der, issuer) = load_or_create_ca(directory)?;
    let now = OffsetDateTime::now_utc();
    let server_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(|error| error.to_string())?;
    let mut parameters = CertificateParams::default();
    parameters.not_before = now - Duration::days(1);
    parameters.not_after = now + Duration::days(365);
    parameters.subject_alt_names = vec![SanType::IpAddress(server_ip)];
    parameters.is_ca = IsCa::ExplicitNoCa;
    parameters.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    parameters.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    parameters.use_authority_key_identifier_extension = true;
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, server_ip.to_string());
    parameters.distinguished_name = distinguished_name;

    let certificate = parameters.signed_by(&server_key, &issuer).map_err(|error| format!("Não foi possível assinar o certificado HTTPS: {error}"))?;
    Ok(TlsMaterial {
        ca_der,
        server_certificate_der: certificate.der().to_vec(),
        server_key_der: server_key.serialize_der(),
    })
}

fn load_or_create_ca(directory: &Path) -> Result<(Vec<u8>, Issuer<'static, KeyPair>), String> {
    fs::create_dir_all(directory).map_err(|error| format!("Não foi possível criar a pasta HTTPS: {error}"))?;
    let certificate_path = directory.join("open-productivity-deck-ca.cer");
    let key_path = directory.join("open-productivity-deck-ca-key.pk8");

    match (certificate_path.exists(), key_path.exists()) {
        (false, false) => create_ca_files(&certificate_path, &key_path)?,
        (true, true) => {}
        _ => return Err("Os arquivos da autoridade certificadora local estão incompletos".into()),
    }

    let ca_der = fs::read(&certificate_path).map_err(|error| format!("Não foi possível ler o certificado local: {error}"))?;
    let key_der = fs::read(&key_path).map_err(|error| format!("Não foi possível ler a chave HTTPS local: {error}"))?;
    let key_pair = KeyPair::try_from(key_der).map_err(|error| format!("A chave HTTPS local é inválida: {error}"))?;
    let certificate = CertificateDer::from_slice(&ca_der);
    let issuer = Issuer::from_ca_cert_der(&certificate, key_pair).map_err(|error| format!("O certificado local é inválido: {error}"))?;
    Ok((ca_der, issuer))
}

fn create_ca_files(certificate_path: &Path, key_path: &Path) -> Result<(), String> {
    let now = OffsetDateTime::now_utc();
    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(|error| error.to_string())?;
    let mut parameters = CertificateParams::default();
    parameters.not_before = now - Duration::days(1);
    parameters.not_after = now + Duration::days(3650);
    parameters.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    parameters.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    parameters.use_authority_key_identifier_extension = true;
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, "Open Productivity Deck Local CA");
    parameters.distinguished_name = distinguished_name;
    let certificate = parameters.self_signed(&key_pair).map_err(|error| error.to_string())?;

    create_new_file(key_path, &key_pair.serialize_der())?;
    create_new_file(certificate_path, certificate.der().as_ref())
}

fn create_new_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("Não foi possível criar {}: {error}", path.display()))?;
    file.write_all(contents).map_err(|error| format!("Não foi possível gravar {}: {error}", path.display()))?;
    file.sync_all().map_err(|error| format!("Não foi possível salvar {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn reuses_ca_when_server_certificate_is_regenerated() {
        let directory = std::env::temp_dir().join(format!("opd-tls-test-{}", Uuid::new_v4()));
        let first = create(&directory, "192.168.1.10".parse().unwrap()).unwrap();
        let second = create(&directory, "192.168.1.11".parse().unwrap()).unwrap();

        assert_eq!(first.ca_der, second.ca_der);
        assert_ne!(first.server_certificate_der, second.server_certificate_der);
        let _ = fs::remove_dir_all(directory);
    }
}
