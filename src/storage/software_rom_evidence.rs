/// What the source ROM entry proves about the scope of its supplied hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RomEvidence {
    WholeFile,
    Unproved,
}

impl RomEvidence {
    pub(super) fn classify(declares_file: bool, name: Option<&str>, nodump: bool) -> Self {
        if declares_file && name.is_some_and(|name| !name.is_empty()) && !nodump {
            Self::WholeFile
        } else {
            Self::Unproved
        }
    }

    pub(super) const fn scope(self) -> &'static str {
        match self {
            Self::WholeFile => "whole_asset",
            Self::Unproved => "unknown",
        }
    }

    pub(super) fn accepts_scope(self, scope: &str) -> bool {
        match self {
            Self::WholeFile => matches!(scope, "whole_asset" | "whole_file"),
            Self::Unproved => scope == "unknown",
        }
    }
}
