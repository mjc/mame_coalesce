use serde::Deserialize;

use crate::{
    logiqx::{AttributePosition, ClrMameProAttribute, RecordLocation, RomCenterAttribute},
    xml_reader::Element,
};

#[derive(Clone, Debug, Deserialize)]
pub struct Header {
    name: String,
    description: Option<String>,
    version: Option<String>,
    date: Option<String>,
    author: Option<String>,
    email: Option<String>,
    homepage: Option<String>,
    url: Option<String>,
    comment: Option<String>,
    category: Option<String>,
    #[serde(skip)]
    clrmamepro: Option<ClrMameProOptions>,
    #[serde(skip)]
    romcenter: Option<RomCenterOptions>,
    #[serde(skip)]
    child_locations: Vec<RecordLocation>,
    #[serde(skip)]
    text_positions: Vec<HeaderTextPosition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum HeaderTextField {
    Name,
    Description,
    Category,
    Version,
    Date,
    Author,
    Email,
    Homepage,
    Url,
    Comment,
}

impl HeaderTextField {
    fn from_element_name(name: &str) -> Option<Self> {
        match name {
            "name" => Some(Self::Name),
            "description" => Some(Self::Description),
            "category" => Some(Self::Category),
            "version" => Some(Self::Version),
            "date" => Some(Self::Date),
            "author" => Some(Self::Author),
            "email" => Some(Self::Email),
            "homepage" => Some(Self::Homepage),
            "url" => Some(Self::Url),
            "comment" => Some(Self::Comment),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) const fn database_value(self) -> i64 {
        self as i64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderTextPosition {
    pub field: HeaderTextField,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug)]
pub struct ClrMameProOptions {
    location: RecordLocation,
    header: Option<String>,
    forcemerging: Option<String>,
    forcenodump: Option<String>,
    forcepacking: Option<String>,
    attribute_positions: Vec<AttributePosition<ClrMameProAttribute>>,
}

#[derive(Clone, Debug)]
pub struct RomCenterOptions {
    location: RecordLocation,
    plugin: Option<String>,
    rommode: Option<String>,
    biosmode: Option<String>,
    samplemode: Option<String>,
    lockrommode: Option<String>,
    lockbiosmode: Option<String>,
    locksamplemode: Option<String>,
    attribute_positions: Vec<AttributePosition<RomCenterAttribute>>,
}

fn option_value(element: &Element, name: &str, values: &[&str]) -> crate::Result<Option<String>> {
    let Some(value) = element.attributes.get(name) else {
        return Ok(None);
    };
    if values.contains(&value.as_str()) {
        Ok(Some(value.clone()))
    } else {
        Err(crate::Error::XmlValidation(format!(
            "invalid {name} value {value:?} on <{}>",
            element.name
        )))
    }
}

impl ClrMameProOptions {
    #[must_use]
    pub fn attribute_positions(&self) -> &[AttributePosition<ClrMameProAttribute>] {
        &self.attribute_positions
    }
    fn from_xml(element: &Element) -> crate::Result<Self> {
        Ok(Self {
            location: element.location,
            header: element.attributes.get("header").cloned(),
            forcemerging: option_value(element, "forcemerging", &["none", "split", "full"])?,
            forcenodump: option_value(element, "forcenodump", &["obsolete", "required", "ignore"])?,
            forcepacking: option_value(element, "forcepacking", &["zip", "unzip"])?,
            attribute_positions: element
                .attributes
                .positions(ClrMameProAttribute::from_name)
                .collect(),
        })
    }

    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
    }

    #[must_use]
    pub fn header(&self) -> Option<&str> {
        self.header.as_deref()
    }
    #[must_use]
    pub fn forcemerging(&self) -> &str {
        self.forcemerging.as_deref().unwrap_or("split")
    }
    #[must_use]
    pub fn forcenodump(&self) -> &str {
        self.forcenodump.as_deref().unwrap_or("obsolete")
    }
    #[must_use]
    pub fn forcepacking(&self) -> &str {
        self.forcepacking.as_deref().unwrap_or("zip")
    }
    #[must_use]
    pub const fn forcemerging_was_explicit(&self) -> bool {
        self.forcemerging.is_some()
    }
    #[must_use]
    pub const fn forcenodump_was_explicit(&self) -> bool {
        self.forcenodump.is_some()
    }
    #[must_use]
    pub const fn forcepacking_was_explicit(&self) -> bool {
        self.forcepacking.is_some()
    }
}

impl RomCenterOptions {
    #[must_use]
    pub fn attribute_positions(&self) -> &[AttributePosition<RomCenterAttribute>] {
        &self.attribute_positions
    }
    fn from_xml(element: &Element) -> crate::Result<Self> {
        Ok(Self {
            location: element.location,
            plugin: element.attributes.get("plugin").cloned(),
            rommode: option_value(element, "rommode", &["merged", "split", "unmerged"])?,
            biosmode: option_value(element, "biosmode", &["merged", "split", "unmerged"])?,
            samplemode: option_value(element, "samplemode", &["merged", "unmerged"])?,
            lockrommode: option_value(element, "lockrommode", &["yes", "no"])?,
            lockbiosmode: option_value(element, "lockbiosmode", &["yes", "no"])?,
            locksamplemode: option_value(element, "locksamplemode", &["yes", "no"])?,
            attribute_positions: element
                .attributes
                .positions(RomCenterAttribute::from_name)
                .collect(),
        })
    }

    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
    }

    #[must_use]
    pub fn plugin(&self) -> Option<&str> {
        self.plugin.as_deref()
    }
    #[must_use]
    pub fn rommode(&self) -> &str {
        self.rommode.as_deref().unwrap_or("split")
    }
    #[must_use]
    pub fn biosmode(&self) -> &str {
        self.biosmode.as_deref().unwrap_or("split")
    }
    #[must_use]
    pub fn samplemode(&self) -> &str {
        self.samplemode.as_deref().unwrap_or("merged")
    }
    #[must_use]
    pub fn lockrommode(&self) -> &str {
        self.lockrommode.as_deref().unwrap_or("no")
    }
    #[must_use]
    pub fn lockbiosmode(&self) -> &str {
        self.lockbiosmode.as_deref().unwrap_or("no")
    }
    #[must_use]
    pub fn locksamplemode(&self) -> &str {
        self.locksamplemode.as_deref().unwrap_or("no")
    }
    #[must_use]
    pub const fn rommode_was_explicit(&self) -> bool {
        self.rommode.is_some()
    }
    #[must_use]
    pub const fn biosmode_was_explicit(&self) -> bool {
        self.biosmode.is_some()
    }
    #[must_use]
    pub const fn samplemode_was_explicit(&self) -> bool {
        self.samplemode.is_some()
    }
    #[must_use]
    pub const fn lockrommode_was_explicit(&self) -> bool {
        self.lockrommode.is_some()
    }
    #[must_use]
    pub const fn lockbiosmode_was_explicit(&self) -> bool {
        self.lockbiosmode.is_some()
    }
    #[must_use]
    pub const fn locksamplemode_was_explicit(&self) -> bool {
        self.locksamplemode.is_some()
    }
}

impl Header {
    pub(crate) fn from_xml(element: &Element) -> crate::Result<Self> {
        let name = element
            .child_text_preserved("name")?
            .ok_or_else(|| crate::Error::XmlValidation("missing required <header><name>".into()))?;
        let mut clrmamepro = None;
        let mut romcenter = None;
        for child in element.children() {
            match child.name.as_str() {
                "clrmamepro"
                    if clrmamepro
                        .replace(ClrMameProOptions::from_xml(child)?)
                        .is_some() =>
                {
                    return Err(crate::Error::XmlValidation(
                        "duplicate <clrmamepro> in <header>".into(),
                    ));
                }
                "romcenter"
                    if romcenter
                        .replace(RomCenterOptions::from_xml(child)?)
                        .is_some() =>
                {
                    return Err(crate::Error::XmlValidation(
                        "duplicate <romcenter> in <header>".into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(Self {
            name,
            description: element.child_text_preserved("description")?,
            version: element.child_text_preserved("version")?,
            date: element.child_text_preserved("date")?,
            author: element.child_text_preserved("author")?,
            email: element.child_text_preserved("email")?,
            homepage: element.child_text_preserved("homepage")?,
            url: element.child_text_preserved("url")?,
            comment: element.child_text_preserved("comment")?,
            category: element.child_text_preserved("category")?,
            clrmamepro,
            romcenter,
            child_locations: element.children().map(|child| child.location).collect(),
            text_positions: element
                .children()
                .enumerate()
                .filter_map(|(source_order, child)| {
                    HeaderTextField::from_element_name(&child.name).map(|field| {
                        HeaderTextPosition {
                            field,
                            source_order,
                            location: child.location,
                        }
                    })
                })
                .collect(),
        })
    }

    /// Position among all header children, including intervening scalar fields.
    #[must_use]
    pub fn child_source_order(&self, location: RecordLocation) -> Option<usize> {
        self.child_locations
            .binary_search_by_key(&(location.line, location.column), |child| {
                (child.line, child.column)
            })
            .ok()
    }

    /// Recognized scalar text elements in source order, including empty values.
    #[must_use]
    pub fn text_positions(&self) -> &[HeaderTextPosition] {
        &self.text_positions
    }

    /// Get a reference to the header's name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the header's homepage.
    #[must_use]
    pub const fn homepage(&self) -> Option<&String> {
        self.homepage.as_ref()
    }

    /// Get a reference to the header's description.
    #[must_use]
    pub const fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    /// Get a reference to the header's version.
    #[must_use]
    pub const fn version(&self) -> Option<&String> {
        self.version.as_ref()
    }

    /// Get a reference to the header's author.
    #[must_use]
    pub const fn author(&self) -> Option<&String> {
        self.author.as_ref()
    }

    /// Get a reference to the header's date.
    #[must_use]
    pub const fn date(&self) -> Option<&String> {
        self.date.as_ref()
    }

    /// Get a reference to the header's email.
    #[must_use]
    pub const fn email(&self) -> Option<&String> {
        self.email.as_ref()
    }

    /// Get a reference to the header's url.
    #[must_use]
    pub const fn url(&self) -> Option<&String> {
        self.url.as_ref()
    }

    /// Get a reference to the header's comment.
    #[must_use]
    pub const fn comment(&self) -> Option<&String> {
        self.comment.as_ref()
    }

    /// Get a reference to the header's category.
    #[must_use]
    pub const fn category(&self) -> Option<&String> {
        self.category.as_ref()
    }

    #[must_use]
    pub const fn clrmamepro_options(&self) -> Option<&ClrMameProOptions> {
        self.clrmamepro.as_ref()
    }

    #[must_use]
    pub const fn romcenter_options(&self) -> Option<&RomCenterOptions> {
        self.romcenter.as_ref()
    }
}
