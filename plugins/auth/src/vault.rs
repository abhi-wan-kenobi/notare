#[derive(Debug, Clone)]
pub struct Vault {
    service: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, strum::AsRefStr, specta::Type)]
pub enum VaultKey {
    #[strum(serialize = "remote-database")]
    #[serde(rename = "remote-database")]
    #[specta(rename = "remote-database")]
    RemoteDatabase,
    #[strum(serialize = "remote-server")]
    #[serde(rename = "remote-server")]
    #[specta(rename = "remote-server")]
    RemoteServer,
}

impl Vault {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    pub fn get(&self, key: VaultKey) -> Result<Option<String>, keyring::Error> {
        let entry = keyring::Entry::new(&self.service, key.as_ref()).unwrap();
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn set(&self, key: VaultKey, value: impl AsRef<str>) -> Result<(), keyring::Error> {
        let entry = keyring::Entry::new(&self.service, key.as_ref()).unwrap();
        entry.set_password(value.as_ref())
    }

    pub fn clear(&self) -> Result<(), keyring::Error> {
        for key in [VaultKey::RemoteDatabase, VaultKey::RemoteServer] {
            let entry = keyring::Entry::new(&self.service, key.as_ref()).unwrap();

            match entry.delete_credential() {
                Ok(_) | Err(keyring::Error::NoEntry) => (),
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}
