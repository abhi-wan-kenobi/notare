use keygen_rs::license::License;

#[derive(Debug, Clone, Default)]
pub struct LicenseState {
    pub key: Option<String>,
    pub license: Option<License>,
    pub valid: bool,
}

pub fn validate() {
    let signed_key = "4F5D3B-0FB8B2-6871BC-5D3EB3-4885B7-V3".to_string();
    let _ = keygen_rs::verify(keygen_rs::license::SchemeCode::Ed25519Sign, &signed_key);
}
