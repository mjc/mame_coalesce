use crate::{logiqx::RecordLocation, xml_reader::Element};

use super::MameBoolean;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSpecificationElement {
    pub element_order: i64,
    pub value: MachineSpecification,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MachineSpecification {
    Sample(Sample),
    Chip(Chip),
    Display(Display),
    Sound(Sound),
    Input(Input),
    Port(Port),
    Adjuster(Adjuster),
    Driver(Driver),
    Feature(Feature),
    Device(Device),
    Slot(Slot),
    SoftwareList(SoftwareList),
    RamOption(RamOption),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub name: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub name: String,
    pub tag: Option<String>,
    pub kind: ChipKind,
    pub clock: Option<String>,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipKind {
    Cpu,
    Audio,
}

impl ChipKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Audio => "audio",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Display {
    pub tag: Option<String>,
    pub kind: DisplayKind,
    pub rotation: Option<DisplayRotation>,
    pub flip_x: MameBoolean,
    pub width: Option<String>,
    pub height: Option<String>,
    pub refresh: String,
    pub pixel_clock: Option<String>,
    pub horizontal_total: Option<String>,
    pub horizontal_blank_end: Option<String>,
    pub horizontal_blank_start: Option<String>,
    pub vertical_total: Option<String>,
    pub vertical_blank_end: Option<String>,
    pub vertical_blank_start: Option<String>,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayKind {
    Raster,
    Vector,
    Lcd,
    Svg,
    Unknown,
}

impl DisplayKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raster => "raster",
            Self::Vector => "vector",
            Self::Lcd => "lcd",
            Self::Svg => "svg",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayRotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl DisplayRotation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deg0 => "0",
            Self::Deg90 => "90",
            Self::Deg180 => "180",
            Self::Deg270 => "270",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sound {
    pub channels: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub service: MameBoolean,
    pub tilt: MameBoolean,
    pub players: String,
    pub coins: Option<String>,
    pub controls: Vec<InputControl>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputControl {
    pub kind: String,
    pub player: Option<String>,
    pub buttons: Option<String>,
    pub minimum: Option<String>,
    pub maximum: Option<String>,
    pub sensitivity: Option<String>,
    pub key_delta: Option<String>,
    pub reverse: MameBoolean,
    pub ways: Option<String>,
    pub ways2: Option<String>,
    pub ways3: Option<String>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    pub tag: String,
    pub analogs: Vec<Analog>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Analog {
    pub mask: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adjuster {
    pub name: String,
    pub default: String,
    pub condition: Option<MachineCondition>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineCondition {
    pub tag: String,
    pub mask: String,
    pub relation: ConditionRelation,
    pub value: String,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionRelation {
    Eq,
    Ne,
    Gt,
    Le,
    Lt,
    Ge,
}

impl ConditionRelation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Gt => "gt",
            Self::Le => "le",
            Self::Lt => "lt",
            Self::Ge => "ge",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Driver {
    pub status: DriverQuality,
    pub emulation: DriverQuality,
    pub cocktail: Option<DriverQuality>,
    pub savestate: SaveState,
    pub requires_artwork: MameBoolean,
    pub unofficial: MameBoolean,
    pub no_sound_hardware: MameBoolean,
    pub incomplete: MameBoolean,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverQuality {
    Good,
    Imperfect,
    Preliminary,
}

impl DriverQuality {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Imperfect => "imperfect",
            Self::Preliminary => "preliminary",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveState {
    Supported,
    Unsupported,
}

impl SaveState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feature {
    pub kind: FeatureKind,
    pub status: Option<FeatureStatus>,
    pub overall: Option<FeatureStatus>,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureStatus {
    Unemulated,
    Imperfect,
}

impl FeatureStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unemulated => "unemulated",
            Self::Imperfect => "imperfect",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Protection,
    Timing,
    Graphics,
    Palette,
    Sound,
    Capture,
    Camera,
    Microphone,
    Controls,
    Keyboard,
    Mouse,
    Media,
    Disk,
    Printer,
    Tape,
    Punch,
    Drum,
    Rom,
    Comms,
    Lan,
    Wan,
}

impl FeatureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Protection => "protection",
            Self::Timing => "timing",
            Self::Graphics => "graphics",
            Self::Palette => "palette",
            Self::Sound => "sound",
            Self::Capture => "capture",
            Self::Camera => "camera",
            Self::Microphone => "microphone",
            Self::Controls => "controls",
            Self::Keyboard => "keyboard",
            Self::Mouse => "mouse",
            Self::Media => "media",
            Self::Disk => "disk",
            Self::Printer => "printer",
            Self::Tape => "tape",
            Self::Punch => "punch",
            Self::Drum => "drum",
            Self::Rom => "rom",
            Self::Comms => "comms",
            Self::Lan => "lan",
            Self::Wan => "wan",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub kind: String,
    pub tag: Option<String>,
    pub fixed_image: Option<String>,
    pub mandatory: Option<String>,
    pub interface: Option<String>,
    pub instance: Option<DeviceInstance>,
    pub extensions: Vec<DeviceExtension>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInstance {
    pub name: String,
    pub brief_name: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceExtension {
    pub name: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub name: String,
    pub options: Vec<SlotOption>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotOption {
    pub name: String,
    pub device_name: String,
    pub is_default: MameBoolean,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareList {
    pub tag: String,
    pub name: String,
    pub status: SoftwareListStatus,
    pub filter: Option<String>,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareListStatus {
    Original,
    Compatible,
}

impl SoftwareListStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Compatible => "compatible",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RamOption {
    pub name: String,
    pub default: Option<String>,
    pub text: String,
    pub location: RecordLocation,
}

fn attribute(node: &Element, name: &str) -> Option<String> {
    node.attributes.get(name).cloned()
}

fn required(node: &Element, name: &str) -> crate::Result<String> {
    node.attributes.get(name).cloned().ok_or_else(|| {
        crate::Error::XmlValidation(format!(
            "<{}> is missing required {name:?} attribute",
            node.name
        ))
    })
}

fn choice<T>(node: &Element, name: &str, variants: &[(&str, T)]) -> crate::Result<T>
where
    T: Copy,
{
    let value = required(node, name)?;
    variants
        .iter()
        .find_map(|(candidate, parsed)| (*candidate == value).then_some(*parsed))
        .ok_or_else(|| {
            crate::Error::XmlValidation(format!("invalid <{}> {name} value {value:?}", node.name))
        })
}

fn boolean(node: &Element, name: &str, default: bool) -> crate::Result<MameBoolean> {
    super::parse_mame_boolean_with_default(node.attributes.get(name), default, name).map(|value| {
        if value {
            MameBoolean::Yes
        } else {
            MameBoolean::No
        }
    })
}

pub(super) fn parse_condition(node: &Element) -> crate::Result<MachineCondition> {
    Ok(MachineCondition {
        tag: required(node, "tag")?,
        mask: required(node, "mask")?,
        relation: choice(
            node,
            "relation",
            &[
                ("eq", ConditionRelation::Eq),
                ("ne", ConditionRelation::Ne),
                ("gt", ConditionRelation::Gt),
                ("le", ConditionRelation::Le),
                ("lt", ConditionRelation::Lt),
                ("ge", ConditionRelation::Ge),
            ],
        )?,
        value: required(node, "value")?,
        location: node.location,
    })
}

fn optional_condition(node: &Element) -> crate::Result<Option<MachineCondition>> {
    node.children()
        .find(|child| child.name == "condition")
        .map(parse_condition)
        .transpose()
}

// Keep the exhaustive schema-to-domain mapping together: every DTD field is
// visible in one match, so omissions are easy to audit against the spec.
#[allow(clippy::too_many_lines)]
pub(super) fn parse_element(node: &Element) -> crate::Result<MachineSpecification> {
    let location = node.location;
    let value = match node.name.as_str() {
        "sample" => MachineSpecification::Sample(Sample {
            name: required(node, "name")?,
            location,
        }),
        "chip" => MachineSpecification::Chip(Chip {
            name: required(node, "name")?,
            tag: attribute(node, "tag"),
            kind: choice(
                node,
                "type",
                &[("cpu", ChipKind::Cpu), ("audio", ChipKind::Audio)],
            )?,
            clock: attribute(node, "clock"),
            location,
        }),
        "display" => MachineSpecification::Display(Display {
            tag: attribute(node, "tag"),
            kind: choice(
                node,
                "type",
                &[
                    ("raster", DisplayKind::Raster),
                    ("vector", DisplayKind::Vector),
                    ("lcd", DisplayKind::Lcd),
                    ("svg", DisplayKind::Svg),
                    ("unknown", DisplayKind::Unknown),
                ],
            )?,
            rotation: node
                .attributes
                .get("rotate")
                .map(|_| {
                    choice(
                        node,
                        "rotate",
                        &[
                            ("0", DisplayRotation::Deg0),
                            ("90", DisplayRotation::Deg90),
                            ("180", DisplayRotation::Deg180),
                            ("270", DisplayRotation::Deg270),
                        ],
                    )
                })
                .transpose()?,
            flip_x: boolean(node, "flipx", false)?,
            width: attribute(node, "width"),
            height: attribute(node, "height"),
            refresh: required(node, "refresh")?,
            pixel_clock: attribute(node, "pixclock"),
            horizontal_total: attribute(node, "htotal"),
            horizontal_blank_end: attribute(node, "hbend"),
            horizontal_blank_start: attribute(node, "hbstart"),
            vertical_total: attribute(node, "vtotal"),
            vertical_blank_end: attribute(node, "vbend"),
            vertical_blank_start: attribute(node, "vbstart"),
            location,
        }),
        "sound" => MachineSpecification::Sound(Sound {
            channels: required(node, "channels")?,
            location,
        }),
        "input" => MachineSpecification::Input(Input {
            service: boolean(node, "service", false)?,
            tilt: boolean(node, "tilt", false)?,
            players: required(node, "players")?,
            coins: attribute(node, "coins"),
            controls: node
                .children()
                .filter(|child| child.name == "control")
                .map(|child| {
                    Ok(InputControl {
                        kind: required(child, "type")?,
                        player: attribute(child, "player"),
                        buttons: attribute(child, "buttons"),
                        minimum: attribute(child, "minimum"),
                        maximum: attribute(child, "maximum"),
                        sensitivity: attribute(child, "sensitivity"),
                        key_delta: attribute(child, "keydelta"),
                        reverse: boolean(child, "reverse", false)?,
                        ways: attribute(child, "ways"),
                        ways2: attribute(child, "ways2"),
                        ways3: attribute(child, "ways3"),
                        location: child.location,
                    })
                })
                .collect::<crate::Result<Vec<_>>>()?,
            location,
        }),
        "port" => MachineSpecification::Port(Port {
            tag: required(node, "tag")?,
            analogs: node
                .children()
                .filter(|child| child.name == "analog")
                .map(|child| {
                    Ok(Analog {
                        mask: required(child, "mask")?,
                        location: child.location,
                    })
                })
                .collect::<crate::Result<Vec<_>>>()?,
            location,
        }),
        "adjuster" => MachineSpecification::Adjuster(Adjuster {
            name: required(node, "name")?,
            default: required(node, "default")?,
            condition: optional_condition(node)?,
            location,
        }),
        "driver" => MachineSpecification::Driver(Driver {
            status: choice(
                node,
                "status",
                &[
                    ("good", DriverQuality::Good),
                    ("imperfect", DriverQuality::Imperfect),
                    ("preliminary", DriverQuality::Preliminary),
                ],
            )?,
            emulation: choice(
                node,
                "emulation",
                &[
                    ("good", DriverQuality::Good),
                    ("imperfect", DriverQuality::Imperfect),
                    ("preliminary", DriverQuality::Preliminary),
                ],
            )?,
            cocktail: node
                .attributes
                .get("cocktail")
                .map(|_| {
                    choice(
                        node,
                        "cocktail",
                        &[
                            ("good", DriverQuality::Good),
                            ("imperfect", DriverQuality::Imperfect),
                            ("preliminary", DriverQuality::Preliminary),
                        ],
                    )
                })
                .transpose()?,
            savestate: choice(
                node,
                "savestate",
                &[
                    ("supported", SaveState::Supported),
                    ("unsupported", SaveState::Unsupported),
                ],
            )?,
            requires_artwork: boolean(node, "requiresartwork", false)?,
            unofficial: boolean(node, "unofficial", false)?,
            no_sound_hardware: boolean(node, "nosoundhardware", false)?,
            incomplete: boolean(node, "incomplete", false)?,
            location,
        }),
        "feature" => MachineSpecification::Feature(Feature {
            kind: choice(
                node,
                "type",
                &[
                    ("protection", FeatureKind::Protection),
                    ("timing", FeatureKind::Timing),
                    ("graphics", FeatureKind::Graphics),
                    ("palette", FeatureKind::Palette),
                    ("sound", FeatureKind::Sound),
                    ("capture", FeatureKind::Capture),
                    ("camera", FeatureKind::Camera),
                    ("microphone", FeatureKind::Microphone),
                    ("controls", FeatureKind::Controls),
                    ("keyboard", FeatureKind::Keyboard),
                    ("mouse", FeatureKind::Mouse),
                    ("media", FeatureKind::Media),
                    ("disk", FeatureKind::Disk),
                    ("printer", FeatureKind::Printer),
                    ("tape", FeatureKind::Tape),
                    ("punch", FeatureKind::Punch),
                    ("drum", FeatureKind::Drum),
                    ("rom", FeatureKind::Rom),
                    ("comms", FeatureKind::Comms),
                    ("lan", FeatureKind::Lan),
                    ("wan", FeatureKind::Wan),
                ],
            )?,
            status: node
                .attributes
                .get("status")
                .map(|_| {
                    choice(
                        node,
                        "status",
                        &[
                            ("unemulated", FeatureStatus::Unemulated),
                            ("imperfect", FeatureStatus::Imperfect),
                        ],
                    )
                })
                .transpose()?,
            overall: node
                .attributes
                .get("overall")
                .map(|_| {
                    choice(
                        node,
                        "overall",
                        &[
                            ("unemulated", FeatureStatus::Unemulated),
                            ("imperfect", FeatureStatus::Imperfect),
                        ],
                    )
                })
                .transpose()?,
            location,
        }),
        "device" => MachineSpecification::Device(Device {
            kind: required(node, "type")?,
            tag: attribute(node, "tag"),
            fixed_image: attribute(node, "fixed_image"),
            mandatory: attribute(node, "mandatory"),
            interface: attribute(node, "interface"),
            instance: node
                .children()
                .find(|child| child.name == "instance")
                .map(|child| -> crate::Result<DeviceInstance> {
                    Ok(DeviceInstance {
                        name: required(child, "name")?,
                        brief_name: required(child, "briefname")?,
                        location: child.location,
                    })
                })
                .transpose()?,
            extensions: node
                .children()
                .filter(|child| child.name == "extension")
                .map(|child| -> crate::Result<DeviceExtension> {
                    Ok(DeviceExtension {
                        name: required(child, "name")?,
                        location: child.location,
                    })
                })
                .collect::<crate::Result<Vec<_>>>()?,
            location,
        }),
        "slot" => MachineSpecification::Slot(Slot {
            name: required(node, "name")?,
            options: node
                .children()
                .filter(|child| child.name == "slotoption")
                .map(|child| {
                    Ok(SlotOption {
                        name: required(child, "name")?,
                        device_name: required(child, "devname")?,
                        is_default: boolean(child, "default", false)?,
                        location: child.location,
                    })
                })
                .collect::<crate::Result<Vec<_>>>()?,
            location,
        }),
        "softwarelist" => MachineSpecification::SoftwareList(SoftwareList {
            tag: required(node, "tag")?,
            name: required(node, "name")?,
            status: choice(
                node,
                "status",
                &[
                    ("original", SoftwareListStatus::Original),
                    ("compatible", SoftwareListStatus::Compatible),
                ],
            )?,
            filter: attribute(node, "filter"),
            location,
        }),
        "ramoption" => MachineSpecification::RamOption(RamOption {
            name: required(node, "name")?,
            default: attribute(node, "default"),
            text: node.direct_text(),
            location,
        }),
        other => {
            return Err(crate::Error::XmlValidation(format!(
                "unexpected MAME specification element <{other}>"
            )));
        }
    };
    Ok(value)
}
