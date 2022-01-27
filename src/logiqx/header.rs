#[derive(Debug, Deserialize)]
pub struct Header {
    name: String,
    description: Option<String>,
    version: Option<String>,
    author: Option<String>,
    homepage: Option<String>,
    url: Option<String>,
}

impl Header {
    /// Get a reference to the header's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the header's homepage.
    pub fn homepage(&self) -> Option<&String> {
        self.homepage.as_ref()
    }

    /// Get a reference to the header's description.
    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    /// Get a reference to the header's version.
    pub fn version(&self) -> Option<&String> {
        self.version.as_ref()
    }

    /// Get a reference to the header's author.
    pub fn author(&self) -> Option<&String> {
        self.author.as_ref()
    }

    /// Get a reference to the header's url.
    pub fn url(&self) -> Option<&String> {
        self.url.as_ref()
    }
}
