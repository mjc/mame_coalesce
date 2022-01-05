#[derive(Debug, Deserialize)]
pub struct Header {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub homepage: Option<String>,
    pub url: String,
}

impl Header {
    /// Get a reference to the header's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the header's description.
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }

    /// Get a reference to the header's version.
    pub fn version(&self) -> &str {
        self.version.as_ref()
    }

    /// Get a reference to the header's author.
    pub fn author(&self) -> &str {
        self.author.as_ref()
    }

    /// Get a reference to the header's url.
    pub fn url(&self) -> &str {
        self.url.as_ref()
    }

    /// Get a reference to the header's homepage.
    pub fn homepage(&self) -> Option<&String> {
        self.homepage.as_ref()
    }
}
