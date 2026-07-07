use thiserror::Error;

#[derive(Debug, Error)]
pub enum MinecraftError {
    #[error("failed to reach the network")]
    Network(#[source] reqwest::Error),

    #[error("received malformed JSON for {context}")]
    Deserialize {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("version '{0}' was not found in the version manifest")]
    VersionNotFound(String),

    #[error(transparent)]
    Download(#[from] downloads::DownloadError),

    #[error("filesystem error: {context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("no Java runtime is available for this instance (system Java not found and no managed runtime for component '{component}')")]
    NoJavaAvailable { component: String },

    #[error("failed to launch the game process")]
    LaunchFailed(#[source] std::io::Error),

    #[error("this version's manifest has no '{0}' downloads.client entry — it may be a server-only or unsupported version")]
    MissingClientDownload(String),

    #[error("malformed library name '{0}'")]
    MalformedLibraryName(String),

    #[error("failed to extract native library archive: {0}")]
    NativeExtraction(String),

    #[error("this instance uses the Fabric loader but no loader version was specified")]
    MissingLoaderVersion,
}

impl From<reqwest::Error> for MinecraftError {
    fn from(err: reqwest::Error) -> Self {
        MinecraftError::Network(err)
    }
}
