use serde::Deserialize;

use super::Rom;
use crate::xml_reader::Element;

#[derive(Debug, Deserialize)]
pub struct Game {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@sourcefile")]
    sourcefile: Option<String>,
    #[serde(rename = "@isbios")]
    isbios: Option<String>,
    #[serde(rename = "@cloneof")]
    cloneof: Option<String>,
    #[serde(rename = "@romof")]
    romof: Option<String>,
    #[serde(rename = "@sampleof")]
    sampleof: Option<String>,
    #[serde(rename = "@board")]
    board: Option<String>,
    #[serde(rename = "@rebuildto")]
    rebuildto: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    year: Option<String>,
    #[serde(default)]
    manufacturer: Option<String>,
    #[serde(rename = "device_ref", default)]
    device_refs: Vec<DeviceRef>,
    #[serde(rename = "rom", default)]
    roms: Vec<Rom>,
}

#[derive(Debug, Deserialize)]
struct DeviceRef {
    #[serde(rename = "@name")]
    name: String,
}

impl Game {
    pub(crate) fn from_xml(element: &Element) -> crate::Result<Self> {
        let mut device_refs = Vec::new();
        let mut roms = Vec::new();
        for child in element.children() {
            match child.name.as_str() {
                "device_ref" => device_refs.push(DeviceRef {
                    name: child.required_attribute("name")?,
                }),
                "rom" => roms.push(Rom::from_xml(child)?),
                _ => {}
            }
        }
        Ok(Self {
            name: element.required_attribute("name")?,
            sourcefile: element.attributes.get("sourcefile").cloned(),
            isbios: element.attributes.get("isbios").cloned(),
            cloneof: element.attributes.get("cloneof").cloned(),
            romof: element.attributes.get("romof").cloned(),
            sampleof: element.attributes.get("sampleof").cloned(),
            board: element.attributes.get("board").cloned(),
            rebuildto: element.attributes.get("rebuildto").cloned(),
            description: element.child_text("description")?,
            year: element.child_text("year")?,
            manufacturer: element.child_text("manufacturer")?,
            device_refs,
            roms,
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
        self.isbios.as_deref()
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
        self.device_refs.iter().map(|device| device.name.as_str())
    }

    #[must_use]
    pub fn roms(&self) -> &[Rom] {
        &self.roms
    }

    #[must_use]
    pub fn cloneof(&self) -> Option<&str> {
        self.cloneof.as_deref()
    }
}
