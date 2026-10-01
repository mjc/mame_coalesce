use super::{Disk, RecordLocation, Rom};
use crate::xml_reader::Element;

#[derive(Debug)]
pub struct Game {
    name: String,
    sourcefile: Option<String>,
    isbios: String,
    isbios_was_explicit: bool,
    cloneof: Option<String>,
    romof: Option<String>,
    sampleof: Option<String>,
    board: Option<String>,
    rebuildto: Option<String>,
    description: Option<String>,
    year: Option<String>,
    manufacturer: Option<String>,
    comments: Vec<NativeComment>,
    releases: Vec<Release>,
    bios_sets: Vec<BiosSet>,
    roms: Vec<Rom>,
    disks: Vec<Disk>,
    samples: Vec<Sample>,
    archives: Vec<Archive>,
    device_refs: Vec<String>,
    child_locations: Vec<RecordLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeComment {
    text: String,
    location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    name: String,
    region: String,
    language: Option<String>,
    date: Option<String>,
    default: String,
    default_was_explicit: bool,
    location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BiosSet {
    name: String,
    description: String,
    default: String,
    default_was_explicit: bool,
    location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    name: String,
    location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archive {
    name: String,
    location: RecordLocation,
}

fn enum_attribute(
    element: &Element,
    name: &str,
    default: &str,
    allowed: &[&str],
) -> crate::Result<String> {
    let value = element.attributes.get(name).map_or(default, String::as_str);
    if allowed.contains(&value) {
        Ok(value.to_owned())
    } else {
        Err(crate::Error::XmlValidation(format!(
            "invalid {name} value {value:?} on <{}>",
            element.name
        )))
    }
}

impl Game {
    /// Element ordinal within this game's source children, including native
    /// metadata and vendor elements rather than only the requested family.
    #[must_use]
    pub fn child_source_order(&self, location: RecordLocation) -> Option<usize> {
        self.child_locations
            .binary_search_by_key(&(location.line, location.column), |child| {
                (child.line, child.column)
            })
            .ok()
    }

    pub(crate) fn from_xml(element: &Element) -> crate::Result<Self> {
        let mut device_refs = Vec::new();
        let mut comments = Vec::new();
        let mut releases = Vec::new();
        let mut bios_sets = Vec::new();
        let mut roms = Vec::new();
        let mut disks = Vec::new();
        let mut samples = Vec::new();
        let mut archives = Vec::new();
        for child in element.children() {
            match child.name.as_str() {
                "device_ref" => device_refs.push(child.required_attribute("name")?),
                "comment" => comments.push(NativeComment {
                    text: child.direct_text(),
                    location: child.location,
                }),
                "release" => releases.push(Release::from_xml(child)?),
                "biosset" => bios_sets.push(BiosSet::from_xml(child)?),
                "rom" => roms.push(Rom::from_xml(child)?),
                "disk" => disks.push(Disk::from_xml(child)?),
                "sample" => samples.push(Sample {
                    name: child.required_attribute("name")?,
                    location: child.location,
                }),
                "archive" => archives.push(Archive {
                    name: child.required_attribute("name")?,
                    location: child.location,
                }),
                _ => {}
            }
        }
        Ok(Self {
            child_locations: element.children().map(|child| child.location).collect(),
            name: element.required_attribute("name")?,
            sourcefile: element.attributes.get("sourcefile").cloned(),
            isbios: enum_attribute(element, "isbios", "no", &["yes", "no"])?,
            isbios_was_explicit: element.attributes.contains_key("isbios"),
            cloneof: element.attributes.get("cloneof").cloned(),
            romof: element.attributes.get("romof").cloned(),
            sampleof: element.attributes.get("sampleof").cloned(),
            board: element.attributes.get("board").cloned(),
            rebuildto: element.attributes.get("rebuildto").cloned(),
            description: element.child_text("description")?,
            year: element.child_text("year")?,
            manufacturer: element.child_text("manufacturer")?,
            comments,
            releases,
            bios_sets,
            roms,
            disks,
            samples,
            archives,
            device_refs,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn sourcefile_opt(&self) -> Option<&str> {
        self.sourcefile.as_deref()
    }
    #[must_use]
    pub fn isbios_opt(&self) -> Option<&str> {
        self.isbios_was_explicit.then_some(&self.isbios)
    }
    #[must_use]
    pub fn isbios_effective(&self) -> &str {
        &self.isbios
    }
    #[must_use]
    pub const fn isbios_was_explicit(&self) -> bool {
        self.isbios_was_explicit
    }
    #[must_use]
    pub fn romof_opt(&self) -> Option<&str> {
        self.romof.as_deref()
    }
    #[must_use]
    pub fn sampleof_opt(&self) -> Option<&str> {
        self.sampleof.as_deref()
    }
    #[must_use]
    pub fn board_opt(&self) -> Option<&str> {
        self.board.as_deref()
    }
    #[must_use]
    pub fn rebuildto_opt(&self) -> Option<&str> {
        self.rebuildto.as_deref()
    }
    #[must_use]
    pub fn description_opt(&self) -> Option<&str> {
        self.description.as_deref()
    }
    #[must_use]
    pub fn year_opt(&self) -> Option<&str> {
        self.year.as_deref()
    }
    #[must_use]
    pub fn manufacturer_opt(&self) -> Option<&str> {
        self.manufacturer.as_deref()
    }
    pub fn device_refs(&self) -> impl Iterator<Item = &str> {
        self.device_refs.iter().map(String::as_str)
    }
    #[must_use]
    pub fn roms(&self) -> &[Rom] {
        &self.roms
    }
    #[must_use]
    pub fn disks(&self) -> &[Disk] {
        &self.disks
    }
    #[must_use]
    pub fn comments(&self) -> &[NativeComment] {
        &self.comments
    }
    #[must_use]
    pub fn releases(&self) -> &[Release] {
        &self.releases
    }
    #[must_use]
    pub fn bios_sets(&self) -> &[BiosSet] {
        &self.bios_sets
    }
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }
    #[must_use]
    pub fn archives(&self) -> &[Archive] {
        &self.archives
    }
    #[must_use]
    pub fn cloneof(&self) -> Option<&str> {
        self.cloneof.as_deref()
    }
}

impl Release {
    fn from_xml(element: &Element) -> crate::Result<Self> {
        Ok(Self {
            name: element.required_attribute("name")?,
            region: element.required_attribute("region")?,
            language: element.attributes.get("language").cloned(),
            date: element.attributes.get("date").cloned(),
            default: enum_attribute(element, "default", "no", &["yes", "no"])?,
            default_was_explicit: element.attributes.contains_key("default"),
            location: element.location,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
    #[must_use]
    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }
    #[must_use]
    pub fn default(&self) -> &str {
        &self.default
    }
    #[must_use]
    pub const fn default_was_explicit(&self) -> bool {
        self.default_was_explicit
    }
    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
    }
}

impl BiosSet {
    fn from_xml(element: &Element) -> crate::Result<Self> {
        Ok(Self {
            name: element.required_attribute("name")?,
            description: element.required_attribute("description")?,
            default: enum_attribute(element, "default", "no", &["yes", "no"])?,
            default_was_explicit: element.attributes.contains_key("default"),
            location: element.location,
        })
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
    #[must_use]
    pub fn default(&self) -> &str {
        &self.default
    }
    #[must_use]
    pub const fn default_was_explicit(&self) -> bool {
        self.default_was_explicit
    }
    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
    }
}

impl NativeComment {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
    }
}

impl Sample {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
    }
}

impl Archive {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
    }
}
