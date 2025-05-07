#![doc = "Certificate utilities for the application."]

// standard imports
use std::fs;
use std::path::Path;

// lib imports
use rcgen::{generate_simple_self_signed, CertifiedKey};

/// Ensure that the certificates exist at the given paths.
pub fn ensure_certificates_exist(
    cert_path: String,
    key_path: String,
) {
    if !Path::new(cert_path.as_str()).exists() || !Path::new(key_path.as_str()).exists() {
        let subject_alt_names = vec!["localhost".to_string()];

        let CertifiedKey { cert, key_pair } =
            generate_simple_self_signed(subject_alt_names).unwrap();

        // create directory tree if necessary
        let cert_dir = Path::new(&cert_path).parent().unwrap();
        let key_dir = Path::new(&key_path).parent().unwrap();
        fs::create_dir_all(cert_dir).expect("Failed to create certificate directory");
        fs::create_dir_all(key_dir).expect("Failed to create private key directory");

        // write the certificate and private key to disk
        fs::write(cert_path, cert.pem()).expect("Failed to write certificate");
        fs::write(key_path, key_pair.serialize_pem()).expect("Failed to write private key");
    }
}
