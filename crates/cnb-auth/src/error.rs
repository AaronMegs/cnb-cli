use cnb_config::ConfigError;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("not authenticated; run `cnb auth login`")]
    NotLoggedIn,
    #[error("no user known for host `{0}`; run `cnb auth login` or pass `--user`")]
    NoUser(String),
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error(transparent)]
    Config(#[from] ConfigError),
}
