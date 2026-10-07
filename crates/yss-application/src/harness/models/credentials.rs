use super::{ModelCredentials, ModelSettingsError};
use yss_harness_contract::SecretCredential;

pub struct SystemModelCredentials;

fn entry(key: &str) -> Result<keyring::Entry, ModelSettingsError> {
    keyring::Entry::new("YssBI.LanguageModels", key).map_err(|_| ModelSettingsError::Credentials)
}

impl ModelCredentials for SystemModelCredentials {
    fn read(&self, key: &str) -> Result<Option<SecretCredential>, ModelSettingsError> {
        match entry(key)?.get_password() {
            Ok(value) => SecretCredential::new(value)
                .map(Some)
                .map_err(|_| ModelSettingsError::Credentials),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(ModelSettingsError::Credentials),
        }
    }

    fn write(&self, key: &str, credential: &SecretCredential) -> Result<(), ModelSettingsError> {
        entry(key)?
            .set_password(credential.expose())
            .map_err(|_| ModelSettingsError::Credentials)
    }

    fn remove(&self, key: &str) -> Result<(), ModelSettingsError> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(ModelSettingsError::Credentials),
        }
    }
}
