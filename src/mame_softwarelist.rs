pub use crate::domain::media::SourceLoadInstruction as LoadInstruction;
pub use crate::xml_reader::AttributePosition;
use crate::xml_reader::attribute_fields;

attribute_fields! { SoftwareWrapperAttribute {
    Build = 0 => "build",
} }

attribute_fields! { SoftwareListAttribute {
    Name = 0 => "name",
    Description = 1 => "description",
} }

attribute_fields! { SoftwareItemAttribute {
    Name = 0 => "name",
    CloneOf = 1 => "cloneof",
    Supported = 2 => "supported",
} }

attribute_fields! { SoftwareNamedValueAttribute {
    Name = 0 => "name",
    Value = 1 => "value",
} }

attribute_fields! { SoftwarePartAttribute {
    Name = 0 => "name",
    Interface = 1 => "interface",
} }

attribute_fields! { SoftwareDataAreaAttribute {
    Name = 0 => "name",
    Size = 1 => "size",
    Width = 2 => "width",
    Endianness = 3 => "endianness",
} }

attribute_fields! { SoftwareDiskAreaAttribute {
    Name = 0 => "name",
} }

attribute_fields! { SoftwareRomAttribute {
    Name = 0 => "name",
    Size = 1 => "size",
    Crc = 2 => "crc",
    Sha1 = 3 => "sha1",
    Offset = 4 => "offset",
    Value = 5 => "value",
    Status = 6 => "status",
    LoadFlag = 7 => "loadflag",
} }

attribute_fields! { SoftwareDiskAttribute {
    Name = 0 => "name",
    Sha1 = 1 => "sha1",
    Status = 2 => "status",
    Writeable = 3 => "writeable",
} }

attribute_fields! { SoftwareDipSwitchAttribute {
    Name = 0 => "name",
    Tag = 1 => "tag",
    Mask = 2 => "mask",
} }

attribute_fields! { SoftwareDipValueAttribute {
    Name = 0 => "name",
    Value = 1 => "value",
    Default = 2 => "default",
} }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareAreaAttributePositions {
    Data(Vec<AttributePosition<SoftwareDataAreaAttribute>>),
    Disk(Vec<AttributePosition<SoftwareDiskAreaAttribute>>),
}

use crate::{
    logiqx::RecordLocation,
    mame::{ExtensionValue, XmlExtension},
    xml_reader::{self, Element, NodeBudget},
};
use quick_xml::events::Event;

macro_rules! string_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            const fn new(value: String) -> Self {
                Self(value)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_identity!(SoftwareListName);
string_identity!(SoftwareItemName);
string_identity!(PartName);
string_identity!(AreaName);
string_identity!(ComponentName);
string_identity!(DipSwitchName);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SupportedStatus {
    #[default]
    Yes,
    Partial,
    No,
}

impl SupportedStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::Partial => "partial",
            Self::No => "no",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedValue {
    pub attribute_positions: Vec<AttributePosition<SoftwareNamedValueAttribute>>,
    pub name: String,
    pub value: Option<String>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareListRootKind {
    SingleList,
    PluralLists,
}

impl SoftwareListRootKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleList => "single_list",
            Self::PluralLists => "plural_lists",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SoftwareTextField {
    Description = 0,
    Year = 1,
    Publisher = 2,
    Notes = 3,
}

impl SoftwareTextField {
    fn from_element_name(name: &str) -> Option<Self> {
        match name {
            "description" => Some(Self::Description),
            "year" => Some(Self::Year),
            "publisher" => Some(Self::Publisher),
            "notes" => Some(Self::Notes),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_code(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareTextPosition {
    pub field: SoftwareTextField,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareListCatalog {
    pub attribute_positions: Vec<AttributePosition<SoftwareWrapperAttribute>>,
    pub root_kind: SoftwareListRootKind,
    pub build: Option<String>,
    pub lists: Vec<SoftwareList>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareList {
    pub attribute_positions: Vec<AttributePosition<SoftwareListAttribute>>,
    pub name: SoftwareListName,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub text_positions: Vec<SoftwareTextPosition>,
    pub source_order: usize,
    pub location: RecordLocation,
    pub items: Vec<SoftwareItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareItem {
    pub attribute_positions: Vec<AttributePosition<SoftwareItemAttribute>>,
    pub name: SoftwareItemName,
    pub clone_of: Option<SoftwareItemName>,
    pub supported: Option<SupportedStatus>,
    pub supported_specified: bool,
    pub description: String,
    pub year: String,
    pub publisher: String,
    pub notes: Option<String>,
    pub text_positions: Vec<SoftwareTextPosition>,
    pub info: Vec<NamedValue>,
    pub shared_features: Vec<NamedValue>,
    pub parts: Vec<SoftwarePart>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwarePart {
    pub attribute_positions: Vec<AttributePosition<SoftwarePartAttribute>>,
    pub name: PartName,
    pub interface: String,
    pub features: Vec<NamedValue>,
    pub dipswitches: Vec<SoftwareDipSwitch>,
    pub areas: Vec<SoftwareArea>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDipSwitch {
    pub attribute_positions: Vec<AttributePosition<SoftwareDipSwitchAttribute>>,
    pub name: DipSwitchName,
    pub tag: String,
    pub mask: String,
    pub values: Vec<SoftwareDipValue>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDipValue {
    pub attribute_positions: Vec<AttributePosition<SoftwareDipValueAttribute>>,
    pub name: String,
    pub value: String,
    pub is_default: bool,
    pub default_specified: bool,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AreaKind {
    Data,
    Disk,
}

impl AreaKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Disk => "disk",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

impl Endianness {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Little => "little",
            Self::Big => "big",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareArea {
    pub attribute_positions: SoftwareAreaAttributePositions,
    pub name: AreaName,
    pub kind: AreaKind,
    pub declared_size: Option<u64>,
    pub declared_size_text: Option<String>,
    pub width: Option<u8>,
    pub width_specified: bool,
    pub endianness: Option<Endianness>,
    pub endianness_specified: bool,
    pub components: Vec<SoftwareComponent>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareComponent {
    Rom(SoftwareRom),
    Disk(SoftwareDisk),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareRom {
    pub attribute_positions: Vec<AttributePosition<SoftwareRomAttribute>>,
    pub name: Option<ComponentName>,
    pub size: Option<u64>,
    pub size_text: Option<String>,
    pub crc: Option<[u8; 4]>,
    pub crc_text: Option<String>,
    pub sha1: Option<[u8; 20]>,
    pub sha1_text: Option<String>,
    pub offset: Option<u64>,
    pub offset_text: Option<String>,
    pub value: Option<String>,
    pub status: Option<DumpStatus>,
    pub status_specified: bool,
    pub load: Option<LoadInstruction>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDisk {
    pub attribute_positions: Vec<AttributePosition<SoftwareDiskAttribute>>,
    pub requirement: crate::disk::DiskRequirement,
    pub status: Option<DumpStatus>,
    pub status_specified: bool,
    pub writeable: Option<bool>,
    pub writeable_specified: bool,
    pub sha1_text: Option<String>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DumpStatus {
    BadDump,
    NoDump,
    #[default]
    Good,
}

impl DumpStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadDump => "baddump",
            Self::NoDump => "nodump",
            Self::Good => "good",
        }
    }
}

impl SoftwareListCatalog {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        struct Collector {
            catalog: SoftwareListCatalog,
            current_list: Option<SoftwareList>,
        }

        let validated = read_with::<_, crate::Error>(
            bytes,
            |header| {
                Ok(Collector {
                    catalog: Self {
                        attribute_positions: header.attribute_positions,
                        root_kind: header.root_kind,
                        build: header.build,
                        lists: Vec::new(),
                        extensions: Vec::new(),
                    },
                    current_list: None,
                })
            },
            |collector, header| {
                collector.current_list = Some(SoftwareList {
                    attribute_positions: Vec::new(),
                    name: header.name,
                    description: header.description,
                    notes: None,
                    text_positions: Vec::new(),
                    source_order: header.source_order,
                    location: header.location,
                    items: Vec::new(),
                });
                Ok(())
            },
            |collector, item| {
                collector
                    .current_list
                    .as_mut()
                    .ok_or_else(|| {
                        crate::Error::XmlValidation(
                            "software-list item arrived without an active list".into(),
                        )
                    })?
                    .items
                    .push(item);
                Ok(())
            },
            |collector, metadata| {
                let mut list = collector.current_list.take().ok_or_else(|| {
                    crate::Error::XmlValidation(
                        "software-list metadata arrived without an active list".into(),
                    )
                })?;
                list.attribute_positions = metadata.attribute_positions;
                list.notes = metadata.notes;
                list.text_positions = metadata.text_positions;
                collector.catalog.lists.push(list);
                Ok(())
            },
            |collector, extension| {
                collector.catalog.extensions.push(extension);
                Ok(())
            },
        )?;
        Ok(validated.into_inner().catalog)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDocumentHeader {
    pub root_kind: SoftwareListRootKind,
    pub build: Option<String>,
    pub attribute_positions: Vec<AttributePosition<SoftwareWrapperAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareListHeader {
    pub name: SoftwareListName,
    pub description: Option<String>,
    pub source_order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareListMetadata {
    pub attribute_positions: Vec<AttributePosition<SoftwareListAttribute>>,
    pub notes: Option<String>,
    pub text_positions: Vec<SoftwareTextPosition>,
}

/// Produced only after the source reader has validated trailing actual EOF.
pub struct ValidatedSoftwareList<S>(S);

impl<S> ValidatedSoftwareList<S> {
    /// Consume the reader-owned valid-EOF proof and return its completed sink.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.0
    }
}

struct SoftwareListCallbacks<'callbacks, S, E> {
    list_start: &'callbacks mut dyn FnMut(&mut S, SoftwareListHeader) -> std::result::Result<(), E>,
    item: &'callbacks mut dyn FnMut(&mut S, SoftwareItem) -> std::result::Result<(), E>,
    list_end: &'callbacks mut dyn FnMut(&mut S, SoftwareListMetadata) -> std::result::Result<(), E>,
    extension: &'callbacks mut dyn FnMut(&mut S, XmlExtension) -> std::result::Result<(), E>,
}

struct SoftwareListParser<'callbacks, 'reader_input, 'position_input, S, E> {
    reader: &'callbacks mut crate::xml_reader::XmlReader<'reader_input>,
    positions: &'callbacks mut xml_reader::PositionMap<'position_input>,
    budget: &'callbacks mut NodeBudget,
    sink: S,
    callbacks: SoftwareListCallbacks<'callbacks, S, E>,
}

/// Stream one software-list document, transferring each complete item to its consumer.
pub fn read_with<S, E: From<crate::Error>>(
    bytes: &[u8],
    start: impl FnOnce(SoftwareDocumentHeader) -> std::result::Result<S, E>,
    mut list_start: impl FnMut(&mut S, SoftwareListHeader) -> std::result::Result<(), E>,
    mut item: impl FnMut(&mut S, SoftwareItem) -> std::result::Result<(), E>,
    mut list_end: impl FnMut(&mut S, SoftwareListMetadata) -> std::result::Result<(), E>,
    mut extension_callback: impl FnMut(&mut S, XmlExtension) -> std::result::Result<(), E>,
) -> std::result::Result<ValidatedSoftwareList<S>, E> {
    xml_reader::with_reader(bytes, |reader, positions| {
        let mut budget = NodeBudget::with_limit(xml_reader::MAX_MAME_XML_NODES);
        let (namespace, event) = loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            if matches!(event, Event::Start(_) | Event::Empty(_)) {
                break (namespace, event);
            }
            if event == Event::Eof {
                return Err(crate::Error::XmlValidation("missing document root".into()).into());
            }
        };
        let (root, empty) = match event {
            Event::Start(start) => (
                xml_reader::element_from_start(
                    reader,
                    namespace,
                    &start,
                    &mut budget,
                    0,
                    positions,
                )?,
                false,
            ),
            Event::Empty(start) => (
                xml_reader::element_from_start(
                    reader,
                    namespace,
                    &start,
                    &mut budget,
                    0,
                    positions,
                )?,
                true,
            ),
            _ => {
                return Err(crate::Error::XmlValidation(
                    "software-list document root was not an opening element".into(),
                )
                .into());
            }
        };

        let document_header = software_document_header(&root)?;
        let sink = start(document_header)?;
        let sink = {
            let mut parser = SoftwareListParser {
                reader,
                positions,
                budget: &mut budget,
                sink,
                callbacks: SoftwareListCallbacks {
                    list_start: &mut list_start,
                    item: &mut item,
                    list_end: &mut list_end,
                    extension: &mut extension_callback,
                },
            };
            parser.read_root(&root, empty)?;
            parser.sink
        };
        finish_softwarelist_document(reader, positions)?;
        Ok(ValidatedSoftwareList(sink))
    })
}

struct SoftwareListProgress {
    notes: Option<String>,
    text_positions: Vec<SoftwareTextPosition>,
    saw_item: bool,
}

impl SoftwareListProgress {
    const fn new() -> Self {
        Self {
            notes: None,
            text_positions: Vec::new(),
            saw_item: false,
        }
    }
}

impl<S, E: From<crate::Error>> SoftwareListParser<'_, '_, '_, S, E> {
    fn read_root(&mut self, root: &Element, empty: bool) -> std::result::Result<(), E> {
        match root.name.as_str() {
            "softwarelist" => self.parse_list(root, empty, 0),
            "softwarelists" => self.parse_plural_root(root, empty),
            _ => Err(crate::Error::XmlValidation(format!(
                "expected <softwarelist> or <softwarelists>, found <{}>",
                root.name
            ))
            .into()),
        }
    }

    fn parse_plural_root(&mut self, root: &Element, empty: bool) -> std::result::Result<(), E> {
        let mut extensions = Vec::new();
        retain_unknown_attributes(root, &["build"], "document", None, &mut extensions);
        self.emit_extensions(extensions)?;
        if !empty {
            self.parse_plural_lists()?;
        }
        Ok(())
    }

    fn parse_plural_lists(&mut self) -> std::result::Result<(), E> {
        let mut source_order = 0;
        loop {
            let (namespace, event) = xml_reader::next(self.reader, self.positions)?;
            let child_order = match &event {
                Event::Start(_) | Event::Empty(_) => {
                    let order = source_order;
                    source_order += 1;
                    Some(order)
                }
                _ => None,
            };
            match event {
                Event::Start(start) if start.local_name().as_ref() == "softwarelist" => {
                    let order = child_order.ok_or_else(|| {
                        crate::Error::XmlValidation("software-list order was not assigned".into())
                    })?;
                    let node = xml_reader::element_from_start(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        1,
                        self.positions,
                    )?;
                    self.parse_list(&node, false, order)?;
                }
                Event::Empty(start) if start.local_name().as_ref() == "softwarelist" => {
                    let order = child_order.ok_or_else(|| {
                        crate::Error::XmlValidation("software-list order was not assigned".into())
                    })?;
                    let node = xml_reader::element_from_start(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        1,
                        self.positions,
                    )?;
                    self.parse_list(&node, true, order)?;
                }
                Event::Start(start) => {
                    let node = xml_reader::read_element(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        1,
                        self.positions,
                    )?;
                    self.emit_extensions(vec![extension("document", None, &node)?])?;
                }
                Event::Empty(start) => {
                    let node = xml_reader::element_from_start(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        1,
                        self.positions,
                    )?;
                    self.emit_extensions(vec![extension("document", None, &node)?])?;
                }
                Event::End(_) => return Ok(()),
                Event::Eof => {
                    return Err(crate::Error::XmlValidation(
                        "unexpected end of input inside <softwarelists>".into(),
                    )
                    .into());
                }
                _ => {}
            }
        }
    }

    fn parse_list(
        &mut self,
        node: &Element,
        empty: bool,
        source_order: usize,
    ) -> std::result::Result<(), E> {
        let name = SoftwareListName::new(required(node, "name")?);
        (self.callbacks.list_start)(
            &mut self.sink,
            SoftwareListHeader {
                name: name.clone(),
                description: node.attributes.get("description").cloned(),
                source_order,
                location: node.location,
            },
        )?;
        let mut extensions = Vec::new();
        retain_unknown_attributes(
            node,
            &["name", "description"],
            "software_list",
            Some(name.as_str()),
            &mut extensions,
        );
        self.emit_extensions(extensions)?;
        let mut progress = SoftwareListProgress::new();
        if !empty {
            self.parse_list_children(&name, &mut progress)?;
        }
        if !progress.saw_item {
            return Err(crate::Error::XmlValidation(format!(
                "software list {:?} has no software items",
                name.as_str()
            ))
            .into());
        }
        (self.callbacks.list_end)(
            &mut self.sink,
            SoftwareListMetadata {
                attribute_positions: node
                    .attributes
                    .positions(SoftwareListAttribute::from_name)
                    .collect(),
                notes: progress.notes,
                text_positions: progress.text_positions,
            },
        )
    }

    fn parse_list_children(
        &mut self,
        list: &SoftwareListName,
        progress: &mut SoftwareListProgress,
    ) -> std::result::Result<(), E> {
        let mut child_order = 0;
        loop {
            let (namespace, event) = xml_reader::next(self.reader, self.positions)?;
            let element_order = match &event {
                Event::Start(_) | Event::Empty(_) => {
                    let order = child_order;
                    child_order += 1;
                    Some(order)
                }
                _ => None,
            };
            match event {
                Event::Start(start)
                    if matches!(start.local_name().as_ref(), "software" | "notes") =>
                {
                    let order = element_order.ok_or_else(|| {
                        crate::Error::XmlValidation(
                            "software-list child order was not assigned".into(),
                        )
                    })?;
                    let node = xml_reader::read_element(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        2,
                        self.positions,
                    )?;
                    self.consume_list_child(&node, list, order, progress)?;
                }
                Event::Empty(start)
                    if matches!(start.local_name().as_ref(), "software" | "notes") =>
                {
                    let order = element_order.ok_or_else(|| {
                        crate::Error::XmlValidation(
                            "software-list child order was not assigned".into(),
                        )
                    })?;
                    let node = xml_reader::element_from_start(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        2,
                        self.positions,
                    )?;
                    self.consume_list_child(&node, list, order, progress)?;
                }
                Event::Start(start) => {
                    let node = xml_reader::read_element(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        2,
                        self.positions,
                    )?;
                    self.emit_extensions(vec![extension(
                        "software_list",
                        Some(list.as_str()),
                        &node,
                    )?])?;
                }
                Event::Empty(start) => {
                    let node = xml_reader::element_from_start(
                        self.reader,
                        namespace,
                        &start,
                        self.budget,
                        2,
                        self.positions,
                    )?;
                    self.emit_extensions(vec![extension(
                        "software_list",
                        Some(list.as_str()),
                        &node,
                    )?])?;
                }
                Event::End(_) => return Ok(()),
                Event::Eof => {
                    return Err(crate::Error::XmlValidation(
                        "unexpected end of input inside <softwarelist>".into(),
                    )
                    .into());
                }
                _ => {}
            }
        }
    }

    fn consume_list_child(
        &mut self,
        node: &Element,
        list: &SoftwareListName,
        source_order: usize,
        progress: &mut SoftwareListProgress,
    ) -> std::result::Result<(), E> {
        if node.name == "notes" {
            let mut extensions = Vec::new();
            retain_unknown_node_content(
                node,
                &[],
                "software_list_notes",
                Some(list.as_str()),
                &mut extensions,
            )?;
            if progress.notes.replace(node.direct_text()).is_some() {
                return Err(crate::Error::XmlValidation(format!(
                    "duplicate software-list notes for {:?}",
                    list.as_str()
                ))
                .into());
            }
            progress.text_positions.push(SoftwareTextPosition {
                field: SoftwareTextField::Notes,
                source_order,
                location: node.location,
            });
            self.emit_extensions(extensions)?;
            return Ok(());
        }

        let mut extensions = Vec::new();
        let software = parse_item(node, list, &mut extensions, source_order)?;
        progress.saw_item = true;
        (self.callbacks.item)(&mut self.sink, software)?;
        self.emit_extensions(extensions)
    }

    fn emit_extensions(&mut self, extensions: Vec<XmlExtension>) -> std::result::Result<(), E> {
        emit_extensions(&mut self.sink, extensions, self.callbacks.extension)
    }
}

fn software_document_header(root: &Element) -> crate::Result<SoftwareDocumentHeader> {
    match root.name.as_str() {
        "softwarelist" => Ok(SoftwareDocumentHeader {
            root_kind: SoftwareListRootKind::SingleList,
            build: None,
            attribute_positions: Vec::new(),
        }),
        "softwarelists" => Ok(SoftwareDocumentHeader {
            root_kind: SoftwareListRootKind::PluralLists,
            build: root.attributes.get("build").cloned(),
            attribute_positions: root
                .attributes
                .positions(SoftwareWrapperAttribute::from_name)
                .collect(),
        }),
        _ => Err(crate::Error::XmlValidation(format!(
            "expected <softwarelist> or <softwarelists>, found <{}>",
            root.name
        ))),
    }
}

fn emit_extensions<S, E>(
    sink: &mut S,
    extensions: Vec<XmlExtension>,
    callback: &mut (impl FnMut(&mut S, XmlExtension) -> std::result::Result<(), E> + ?Sized),
) -> std::result::Result<(), E> {
    for extension in extensions {
        callback(sink, extension)?;
    }
    Ok(())
}

fn finish_softwarelist_document(
    reader: &mut crate::xml_reader::XmlReader<'_>,
    positions: &mut xml_reader::PositionMap<'_>,
) -> crate::Result<()> {
    loop {
        match xml_reader::next(reader, positions)?.1 {
            Event::Eof => return Ok(()),
            Event::Text(text) if text.xml10_content().trim().is_empty() => {}
            Event::Comment(_) | Event::PI(_) => {}
            _ => {
                return Err(crate::Error::XmlValidation(
                    "content after the document root".into(),
                ));
            }
        }
    }
}

fn parse_item(
    node: &Element,
    list: &SoftwareListName,
    extensions: &mut Vec<XmlExtension>,
    source_order: usize,
) -> crate::Result<SoftwareItem> {
    let name = SoftwareItemName::new(required(node, "name")?);
    let record = format!("{}:{}", list.as_str(), name.as_str());
    retain_unknown_attributes(
        node,
        &["name", "cloneof", "supported"],
        "software_item",
        Some(&record),
        extensions,
    );
    let clone_of = node
        .attributes
        .get("cloneof")
        .cloned()
        .map(SoftwareItemName::new);
    let supported = match node.attributes.get("supported").map(String::as_str) {
        None | Some("yes") => Some(SupportedStatus::Yes),
        Some("partial") => Some(SupportedStatus::Partial),
        Some("no") => Some(SupportedStatus::No),
        Some(other) => {
            return Err(invalid_value(node, &record, "supported", other));
        }
    };
    let mut description = None;
    let mut year = None;
    let mut publisher = None;
    let mut notes = None;
    let mut info = Vec::new();
    let mut shared_features = Vec::new();
    let mut parts = Vec::new();
    let mut text_positions = Vec::new();
    for (child_order, child) in node.children().enumerate() {
        if let Some(field) = SoftwareTextField::from_element_name(&child.name) {
            let slot = match field {
                SoftwareTextField::Description => &mut description,
                SoftwareTextField::Year => &mut year,
                SoftwareTextField::Publisher => &mut publisher,
                SoftwareTextField::Notes => &mut notes,
            };
            if field == SoftwareTextField::Notes {
                set_once(slot, child, &name)?;
                retain_unknown_node_content(
                    child,
                    &[],
                    "software_item_notes",
                    Some(&record),
                    extensions,
                )?;
            } else {
                set_item_text(slot, child, &name, &record, extensions)?;
            }
            text_positions.push(SoftwareTextPosition {
                field,
                source_order: child_order,
                location: child.location,
            });
            continue;
        }
        match child.name.as_str() {
            "info" | "sharedfeat" => {
                let (values, owner) = if child.name == "info" {
                    (&mut info, "software_info")
                } else {
                    (&mut shared_features, "software_shared_feature")
                };
                values.push(named_value(child, child_order)?);
                retain_unknown_attributes(
                    child,
                    &["name", "value"],
                    owner,
                    Some(&record),
                    extensions,
                );
                retain_child_elements(child, owner, Some(&record), extensions)?;
            }
            "part" => {
                parts.push(parse_part(child, &record, extensions, child_order)?);
            }
            _ => extensions.push(extension("software_item", Some(&record), child)?),
        }
    }
    Ok(SoftwareItem {
        attribute_positions: node
            .attributes
            .positions(SoftwareItemAttribute::from_name)
            .collect(),
        name,
        clone_of,
        supported,
        supported_specified: node.attributes.contains_key("supported"),
        description: required_text(description, "description", &record)?,
        year: required_text(year, "year", &record)?,
        publisher: required_text(publisher, "publisher", &record)?,
        notes,
        text_positions,
        info,
        shared_features,
        parts,
        source_order,
        location: node.location,
    })
}

fn parse_part(
    node: &Element,
    item_record: &str,
    extensions: &mut Vec<XmlExtension>,
    source_order: usize,
) -> crate::Result<SoftwarePart> {
    let name = PartName::new(required(node, "name")?);
    let record = format!("{item_record}:{}", name.as_str());
    let interface = required(node, "interface")?;
    retain_unknown_attributes(
        node,
        &["name", "interface"],
        "software_part",
        Some(&record),
        extensions,
    );
    let mut features = Vec::new();
    let mut dipswitches = Vec::new();
    let mut areas = Vec::new();
    for (child_order, child) in node.children().enumerate() {
        match child.name.as_str() {
            "feature" => {
                features.push(named_value(child, child_order)?);
                retain_unknown_attributes(
                    child,
                    &["name", "value"],
                    "software_part_feature",
                    Some(&record),
                    extensions,
                );
                retain_child_elements(child, "software_part_feature", Some(&record), extensions)?;
            }
            "dataarea" | "diskarea" => {
                let area = parse_area(child, &record, extensions, child_order)?;
                areas.push(area);
            }
            "dipswitch" => {
                dipswitches.push(parse_dipswitch(child, &record, extensions, child_order)?);
            }
            _ => extensions.push(extension("software_part", Some(&record), child)?),
        }
    }
    Ok(SoftwarePart {
        attribute_positions: node
            .attributes
            .positions(SoftwarePartAttribute::from_name)
            .collect(),
        name,
        interface,
        features,
        dipswitches,
        areas,
        source_order,
        location: node.location,
    })
}

fn parse_dipswitch(
    node: &Element,
    part_record: &str,
    extensions: &mut Vec<XmlExtension>,
    source_order: usize,
) -> crate::Result<SoftwareDipSwitch> {
    let name = DipSwitchName::new(required(node, "name")?);
    let record = format!("{part_record}:{}", name.as_str());
    retain_unknown_attributes(
        node,
        &["name", "tag", "mask"],
        "software_dipswitch",
        Some(&record),
        extensions,
    );
    let mut values = Vec::new();
    for (child_order, child) in node.children().enumerate() {
        if child.name == "dipvalue" {
            retain_unknown_attributes(
                child,
                &["name", "value", "default"],
                "software_dipvalue",
                Some(&record),
                extensions,
            );
            retain_child_elements(child, "software_dipvalue", Some(&record), extensions)?;
            let default_specified = child.attributes.contains_key("default");
            let is_default = match child.attributes.get("default").map(String::as_str) {
                None | Some("no") => false,
                Some("yes") => true,
                Some(other) => return Err(invalid_value(child, &record, "default", other)),
            };
            values.push(SoftwareDipValue {
                attribute_positions: child
                    .attributes
                    .positions(SoftwareDipValueAttribute::from_name)
                    .collect(),
                name: required(child, "name")?,
                value: required(child, "value")?,
                is_default,
                default_specified,
                source_order: child_order,
                location: child.location,
            });
        } else {
            extensions.push(extension("software_dipswitch", Some(&record), child)?);
        }
    }
    Ok(SoftwareDipSwitch {
        attribute_positions: node
            .attributes
            .positions(SoftwareDipSwitchAttribute::from_name)
            .collect(),
        name,
        tag: required(node, "tag")?,
        mask: required(node, "mask")?,
        values,
        source_order,
        location: node.location,
    })
}

fn parse_area(
    node: &Element,
    part_record: &str,
    extensions: &mut Vec<XmlExtension>,
    source_order: usize,
) -> crate::Result<SoftwareArea> {
    let kind = if node.name == "dataarea" {
        AreaKind::Data
    } else {
        AreaKind::Disk
    };
    let name = AreaName::new(required(node, "name")?);
    let record = format!("{part_record}:{}:{}", kind.as_str(), name.as_str());
    let known_attributes: &[&str] = match kind {
        AreaKind::Data => &["name", "size", "width", "endianness"],
        AreaKind::Disk => &["name"],
    };
    retain_unknown_attributes(
        node,
        known_attributes,
        "software_area",
        Some(&record),
        extensions,
    );
    let declared_size_text = match kind {
        AreaKind::Data => Some(required(node, "size")?),
        AreaKind::Disk => None,
    };
    let width_specified = kind == AreaKind::Data && node.attributes.contains_key("width");
    let endianness_specified = kind == AreaKind::Data && node.attributes.contains_key("endianness");
    let (declared_size, width, endianness, component_name) = match kind {
        AreaKind::Data => (
            declared_size_text
                .as_deref()
                .and_then(|value| parse_number(value).ok()),
            Some(
                node.attributes
                    .get("width")
                    .map(|value| parse_width(node, &record, value))
                    .transpose()?
                    .unwrap_or(8),
            ),
            Some(
                node.attributes
                    .get("endianness")
                    .map(|value| parse_endianness(node, &record, value))
                    .transpose()?
                    .unwrap_or(Endianness::Little),
            ),
            "rom",
        ),
        AreaKind::Disk => (None, None, None, "disk"),
    };
    let mut components = Vec::new();
    for (child_order, child) in node.children().enumerate() {
        if child.name == component_name {
            let component = if kind == AreaKind::Data {
                SoftwareComponent::Rom(parse_rom(child, &record, extensions, child_order)?)
            } else {
                SoftwareComponent::Disk(parse_disk(child, &record, extensions, child_order)?)
            };
            components.push(component);
        } else {
            extensions.push(extension("software_area", Some(&record), child)?);
        }
    }
    Ok(SoftwareArea {
        attribute_positions: match kind {
            AreaKind::Data => SoftwareAreaAttributePositions::Data(
                node.attributes
                    .positions(SoftwareDataAreaAttribute::from_name)
                    .collect(),
            ),
            AreaKind::Disk => SoftwareAreaAttributePositions::Disk(
                node.attributes
                    .positions(SoftwareDiskAreaAttribute::from_name)
                    .collect(),
            ),
        },
        name,
        kind,
        declared_size,
        declared_size_text,
        width,
        width_specified,
        endianness,
        endianness_specified,
        components,
        source_order,
        location: node.location,
    })
}

fn parse_rom(
    node: &Element,
    record: &str,
    extensions: &mut Vec<XmlExtension>,
    source_order: usize,
) -> crate::Result<SoftwareRom> {
    retain_unknown_attributes(
        node,
        &[
            "name", "size", "crc", "sha1", "offset", "value", "status", "loadflag",
        ],
        "software_rom",
        Some(record),
        extensions,
    );
    for child in node.children() {
        extensions.push(extension("software_rom", Some(record), child)?);
    }
    let size_text = node.attributes.get("size").cloned();
    let offset_text = node.attributes.get("offset").cloned();
    let crc_text = node.attributes.get("crc").cloned();
    let sha1_text = node.attributes.get("sha1").cloned();
    let status_specified = node.attributes.contains_key("status");
    Ok(SoftwareRom {
        attribute_positions: node
            .attributes
            .positions(SoftwareRomAttribute::from_name)
            .collect(),
        name: node.attributes.get("name").cloned().map(ComponentName::new),
        size: parse_optional_number(node, "size"),
        size_text,
        crc: parse_digest(node, "crc"),
        crc_text,
        sha1: parse_digest(node, "sha1"),
        sha1_text,
        offset: parse_optional_number(node, "offset"),
        offset_text,
        value: node.attributes.get("value").cloned(),
        status: Some(parse_status(node, record)?.unwrap_or_default()),
        status_specified,
        load: parse_load(node, record)?,
        source_order,
        location: node.location,
    })
}

fn parse_disk(
    node: &Element,
    record: &str,
    extensions: &mut Vec<XmlExtension>,
    source_order: usize,
) -> crate::Result<SoftwareDisk> {
    retain_unknown_attributes(
        node,
        &["name", "sha1", "status", "writeable"],
        "software_disk",
        Some(record),
        extensions,
    );
    retain_child_elements(node, "software_disk", Some(record), extensions)?;
    let writeable_specified = node.attributes.contains_key("writeable");
    let writeable = match node.attributes.get("writeable").map(String::as_str) {
        None => None,
        Some("no") => Some(false),
        Some("yes") => Some(true),
        Some(other) => return Err(invalid_value(node, record, "writeable", other)),
    };
    let name = required(node, "name")?;
    let sha1_text = node.attributes.get("sha1").cloned();
    let sha1 = parse_digest(node, "sha1");
    let status_specified = node.attributes.contains_key("status");
    Ok(SoftwareDisk {
        attribute_positions: node
            .attributes
            .positions(SoftwareDiskAttribute::from_name)
            .collect(),
        requirement: crate::disk::DiskRequirement::new(
            crate::disk::DiskName::new(name),
            sha1.map(crate::disk::DiskIdentitySha1::new),
            crate::disk::DiskDigestScope::ChdHeaderSha1,
        ),
        status: Some(parse_status(node, record)?.unwrap_or_default()),
        status_specified,
        writeable: Some(writeable.unwrap_or(false)),
        writeable_specified,
        sha1_text,
        source_order,
        location: node.location,
    })
}

fn parse_status(node: &Element, record: &str) -> crate::Result<Option<DumpStatus>> {
    match node.attributes.get("status").map(String::as_str) {
        None => Ok(None),
        Some("good") => Ok(Some(DumpStatus::Good)),
        Some("baddump") => Ok(Some(DumpStatus::BadDump)),
        Some("nodump") => Ok(Some(DumpStatus::NoDump)),
        Some(other) => Err(invalid_value(node, record, "status", other)),
    }
}

fn parse_load(node: &Element, record: &str) -> crate::Result<Option<LoadInstruction>> {
    node.attributes
        .get("loadflag")
        .map(|value| {
            LoadInstruction::from_source_name(value)
                .ok_or_else(|| invalid_value(node, record, "loadflag", value))
        })
        .transpose()
}

fn parse_width(node: &Element, record: &str, value: &str) -> crate::Result<u8> {
    match value {
        "8" => Ok(8),
        "16" => Ok(16),
        "32" => Ok(32),
        "64" => Ok(64),
        other => Err(invalid_value(node, record, "width", other)),
    }
}

fn parse_endianness(node: &Element, record: &str, value: &str) -> crate::Result<Endianness> {
    match value {
        "little" => Ok(Endianness::Little),
        "big" => Ok(Endianness::Big),
        other => Err(invalid_value(node, record, "endianness", other)),
    }
}

fn parse_optional_number(node: &Element, name: &str) -> Option<u64> {
    node.attributes
        .get(name)
        .and_then(|value| parse_number(value).ok())
}

pub fn parse_number(value: &str) -> crate::Result<u64> {
    let invalid = || crate::Error::XmlValidation(format!("invalid unsigned integer {value:?}"));
    if value.contains('+') {
        return Err(invalid());
    }
    let parsed = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => u64::from_str_radix(hex, 16),
        None if value.len() > 1 && value.starts_with('0') => u64::from_str_radix(value, 8),
        None => value.parse(),
    }
    .map_err(|_| invalid())?;
    i64::try_from(parsed).map_err(|_| {
        crate::Error::XmlValidation(format!("integer exceeds supported storage range {value:?}"))
    })?;
    Ok(parsed)
}

#[cfg(test)]
mod unsigned_number_tests {
    use super::parse_number;

    #[test]
    fn unsigned_numbers_reject_signs_before_and_after_radix_prefixes() {
        for value in [
            "+010", "+08", "+2", "+0x2", "0x+2", "0X+2", "0+2", "-0", "-2", "0x-2",
        ] {
            assert!(
                parse_number(value).is_err(),
                "signed token {value:?} must remain unresolved"
            );
        }
    }
}

fn parse_digest<const N: usize>(node: &Element, field: &str) -> Option<[u8; N]> {
    let value = node.attributes.get(field)?;
    if value.is_empty() {
        return None;
    }
    let Ok(bytes) = hex::decode(value) else {
        return None;
    };
    let Ok(digest) = bytes.try_into() else {
        return None;
    };
    Some(digest)
}

fn named_value(node: &Element, source_order: usize) -> crate::Result<NamedValue> {
    Ok(NamedValue {
        attribute_positions: node
            .attributes
            .positions(SoftwareNamedValueAttribute::from_name)
            .collect(),
        name: required(node, "name")?,
        value: node.attributes.get("value").cloned(),
        source_order,
        location: node.location,
    })
}

fn set_once(
    field: &mut Option<String>,
    node: &Element,
    item: &SoftwareItemName,
) -> crate::Result<()> {
    if field.replace(node.direct_text()).is_some() {
        return Err(crate::Error::XmlValidation(format!(
            "duplicate {} field for software {:?}",
            node.name,
            item.as_str()
        )));
    }
    Ok(())
}

fn retain_child_elements(
    node: &Element,
    record_kind: &str,
    record_name: Option<&str>,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<()> {
    for child in node.children() {
        extensions.push(extension(record_kind, record_name, child)?);
    }
    Ok(())
}

fn retain_unknown_node_content(
    node: &Element,
    known_attributes: &[&str],
    record_kind: &str,
    record_name: Option<&str>,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<()> {
    retain_unknown_attributes(node, known_attributes, record_kind, record_name, extensions);
    retain_child_elements(node, record_kind, record_name, extensions)
}

fn set_item_text(
    field: &mut Option<String>,
    node: &Element,
    item: &SoftwareItemName,
    record: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<()> {
    set_once(field, node, item)?;
    retain_unknown_node_content(node, &[], "software_item", Some(record), extensions)
}

fn required_text(value: Option<String>, field: &str, record: &str) -> crate::Result<String> {
    value.ok_or_else(|| {
        crate::Error::XmlValidation(format!("missing required {field} for software {record:?}"))
    })
}

fn required(node: &Element, field: &str) -> crate::Result<String> {
    node.attributes
        .get(field)
        .cloned()
        .ok_or_else(|| crate::Error::CatalogParse {
            message: format!("missing required {field} attribute on <{}>", node.name),
            record_kind: Some(node.name.clone()),
            record_name: node.attributes.get("name").cloned(),
            line: Some(node.location.line),
            column: Some(node.location.column),
            excerpt: None,
            coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
        })
}

fn invalid_value(node: &Element, record: &str, field: &str, value: &str) -> crate::Error {
    crate::Error::CatalogParse {
        message: format!(
            "invalid {field} value {value:?} on <{}> {record:?}",
            node.name
        ),
        record_kind: Some(node.name.clone()),
        record_name: Some(record.to_owned()),
        line: Some(node.location.line),
        column: Some(node.location.column),
        excerpt: None,
        coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
    }
}

fn extension(
    record_kind: &str,
    record_name: Option<&str>,
    node: &Element,
) -> crate::Result<XmlExtension> {
    let value = ExtensionValue::encode(node)?;
    Ok(XmlExtension {
        record_kind: record_kind.into(),
        record_name: record_name.map(str::to_owned),
        field_name: format!("element:{}", node.name),
        namespace_uri: None,
        value,
        location: node.location,
    })
}

fn retain_unknown_attributes(
    node: &Element,
    known: &[&str],
    record_kind: &str,
    record_name: Option<&str>,
    extensions: &mut Vec<XmlExtension>,
) {
    for (name, raw_value) in &node.attributes {
        if known.contains(&name.as_str()) {
            continue;
        }
        let (field_name, namespace_uri) = name
            .strip_prefix('{')
            .and_then(|name| name.split_once('}'))
            .map_or_else(
                || (format!("@{name}"), None),
                |(namespace, local_name)| (format!("@{local_name}"), Some(namespace.to_owned())),
            );
        extensions.push(XmlExtension {
            record_kind: record_kind.into(),
            record_name: record_name.map(str::to_owned),
            field_name,
            namespace_uri,
            value: serde_json::json!(raw_value).into(),
            location: node.location,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at<T>(values: &[T], index: usize) -> crate::Result<&T> {
        values.get(index).ok_or_else(|| {
            crate::Error::InvalidPath(format!("software-list fixture is missing item {index}"))
        })
    }

    fn as_rom(component: &SoftwareComponent) -> crate::Result<&SoftwareRom> {
        match component {
            SoftwareComponent::Rom(rom) => Ok(rom),
            SoftwareComponent::Disk(_) => Err(crate::Error::InvalidPath(
                "expected ROM component in software-list fixture".into(),
            )),
        }
    }

    fn disk(component: &SoftwareComponent) -> crate::Result<&SoftwareDisk> {
        match component {
            SoftwareComponent::Disk(disk) => Ok(disk),
            SoftwareComponent::Rom(_) => Err(crate::Error::InvalidPath(
                "expected disk component in software-list fixture".into(),
            )),
        }
    }

    #[test]
    fn fixture_preserves_list_scoped_ids_nested_order_loads_and_clone_relationships()
    -> crate::Result<()> {
        let bytes = include_bytes!("../fixtures/catalog/mame/software-list.xml");
        let catalog = SoftwareListCatalog::parse(bytes)?;
        assert_eq!(catalog.build.as_deref(), Some("0.289-synthetic"));
        assert_eq!(catalog.lists.len(), 2);
        let list = at(&catalog.lists, 0)?;
        assert_eq!(list.name.as_str(), "demo_cart");
        assert_eq!(list.items.len(), 2);
        let item = at(&list.items, 0)?;
        assert_eq!(item.name.as_str(), "demo_game");
        assert_eq!(
            item.clone_of.as_ref().map(SoftwareItemName::as_str),
            Some("demo_original")
        );
        assert_eq!(item.supported, Some(SupportedStatus::Partial));
        assert_eq!(
            item.info
                .iter()
                .filter(|value| value.name == "language")
                .count(),
            2
        );
        assert_eq!(at(&item.shared_features, 0)?.name, "compatibility");
        assert_eq!(item.parts.len(), 2);
        let cart = at(&item.parts, 0)?;
        assert_eq!(cart.interface, "demo_cart");
        assert_eq!(cart.areas.len(), 2);
        let program = at(&cart.areas, 0)?;
        assert_eq!(program.kind, AreaKind::Data);
        assert_eq!(program.name.as_str(), "program");
        assert_eq!(program.declared_size, Some(32));
        assert_eq!(program.width, Some(16));
        assert_eq!(program.endianness, Some(Endianness::Big));
        assert_eq!(program.components.len(), 2);
        let rom = as_rom(at(&program.components, 0)?)?;
        assert_eq!(
            rom.name.as_ref().map(ComponentName::as_str),
            Some("program.bin")
        );
        assert_eq!(rom.load, Some(LoadInstruction::Load16WordSwap));
        assert_eq!(rom.offset, Some(0));
        let no_dump = as_rom(at(&program.components, 1)?)?;
        assert_eq!(no_dump.status, Some(DumpStatus::NoDump));
        assert_eq!(no_dump.sha1, None);
        assert_eq!(no_dump.load, Some(LoadInstruction::Continue));
        let media = at(&cart.areas, 1)?;
        let disk = disk(at(&media.components, 0)?)?;
        assert_eq!(disk.writeable, Some(true));
        let manual_area = at(&at(&item.parts, 1)?.areas, 0)?;
        assert_eq!(manual_area.width, Some(8));
        assert_eq!(manual_area.endianness, Some(Endianness::Little));
        assert!(item.location.line < cart.location.line);
        assert!(cart.location.line < rom.location.line);
        let second_list = at(&catalog.lists, 1)?;
        assert_eq!(at(&second_list.items, 0)?.name.as_str(), "demo_game");
        assert_eq!(second_list.name.as_str(), "demo_flop");
        assert!(catalog.extensions.iter().any(|extension| {
            extension.field_name == "element:future-policy"
                && extension.record_name.as_deref() == Some("demo_cart:demo_game")
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.field_name == "@future-flag"
                && extension.value == serde_json::json!("retained").into()
                && extension.record_name.as_deref() == Some("demo_cart:demo_game")
        }));
        Ok(())
    }

    #[test]
    fn parser_preserves_repeated_list_item_and_part_names_in_source_order() -> crate::Result<()> {
        let single = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert_eq!(SoftwareListCatalog::parse(single)?.lists.len(), 1);

        let duplicate = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software><software name="game"><description>Game 2</description><year>2001</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(duplicate)?;
        assert_eq!(catalog.lists[0].items.len(), 2);
        assert_eq!(catalog.lists[0].items[0].source_order, 0);
        assert_eq!(catalog.lists[0].items[1].source_order, 1);

        let duplicate_names = br#"<softwarelists><softwarelist name="one"><software name="game"><description>One</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/><part name="cart" interface="cart"/></software></softwarelist><softwarelist name="one"><software name="game"><description>Two</description><year>2001</year><publisher>Pub</publisher></software></softwarelist></softwarelists>"#;
        let catalog = SoftwareListCatalog::parse(duplicate_names)?;
        assert_eq!(catalog.lists.len(), 2);
        assert_eq!(
            catalog.lists[0].name.as_str(),
            catalog.lists[1].name.as_str()
        );
        assert_eq!(catalog.lists[0].source_order, 0);
        assert_eq!(catalog.lists[1].source_order, 1);
        assert_eq!(catalog.lists[0].items[0].parts.len(), 2);
        assert_eq!(catalog.lists[0].items[0].parts[0].source_order, 3);
        assert_eq!(catalog.lists[0].items[0].parts[1].source_order, 4);
        Ok(())
    }

    #[test]
    fn parser_rejects_duplicate_notes_instead_of_dropping_the_first() {
        let xml = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><notes>First</notes><notes>Second</notes><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(xml).is_err());
    }

    #[test]
    fn parser_accepts_metadata_only_software_without_parts() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert!(at(&at(&catalog.lists, 0)?.items, 0)?.parts.is_empty());
        Ok(())
    }

    #[test]
    fn parser_accepts_empty_aggregate_but_not_empty_individual_lists() -> crate::Result<()> {
        let catalog = SoftwareListCatalog::parse(b"<softwarelists/>")?;
        assert!(catalog.lists.is_empty());
        assert!(SoftwareListCatalog::parse(b"<softwarelist name=\"empty\"/>").is_err());
        Ok(())
    }

    #[test]
    fn parser_retains_single_list_attributes_and_rom_extensions() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one" future-root="kept"><software name="game"><description future-text-attribute="retained">Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="game.bin"><future-component-claim value="kept"/></rom><rom loadflag="continue"/></dataarea><diskarea name="media"><disk name="game-disk"><future-disk-claim value="also-kept"/></disk></diskarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert_eq!(
            catalog
                .extensions
                .iter()
                .filter(|extension| {
                    extension.field_name == "@future-root"
                        && extension.value == serde_json::json!("kept").into()
                })
                .count(),
            1
        );
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_list"
                && extension.field_name == "@future-root"
                && extension.value == serde_json::json!("kept").into()
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_item"
                && extension.field_name == "@future-text-attribute"
                && extension.value == serde_json::json!("retained").into()
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_rom"
                && extension.field_name == "element:future-component-claim"
                && serde_json::from_str::<serde_json::Value>(extension.value.as_str())
                    .is_ok_and(|value| value["attributes"]["value"] == "kept")
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_disk"
                && extension.field_name == "element:future-disk-claim"
                && serde_json::from_str::<serde_json::Value>(extension.value.as_str())
                    .is_ok_and(|value| value["attributes"]["value"] == "also-kept")
        }));
        Ok(())
    }

    #[test]
    fn parser_preserves_spec_valid_empty_rom_records_and_rejects_out_of_range_numbers()
    -> crate::Result<()> {
        let empty_rom = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom/></dataarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(empty_rom)?;
        let rom = as_rom(
            &at(
                &at(&at(&at(&catalog.lists, 0)?.items, 0)?.parts, 0)?.areas,
                0,
            )?
            .components[0],
        )?;
        assert_eq!(rom.name, None);
        assert_eq!(rom.status, Some(DumpStatus::Good));
        let empty_name = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name=""/></dataarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(empty_name)?;
        let rom = as_rom(
            &at(
                &at(&at(&at(&catalog.lists, 0)?.items, 0)?.parts, 0)?.areas,
                0,
            )?
            .components[0],
        )?;
        assert_eq!(rom.name.as_ref().map(ComponentName::as_str), Some(""));
        let unnamed_load = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom loadflag="continue"/></dataarea></part></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(unnamed_load).is_ok());
        assert!(parse_number("9223372036854775808").is_err());
        assert!(parse_number("0x8000000000000000").is_err());
        Ok(())
    }

    #[test]
    fn parser_preserves_unresolved_clone_and_optional_named_values() -> crate::Result<()> {
        let partial = br#"<softwarelist name="partial"><software name="clone" cloneof="omitted_parent" supported="partial"><description>Clone</description><year>2000</year><publisher>Pub</publisher><info name="language"/><sharedfeat name="compatibility"/><part name="cart" interface="cart"/></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(partial)?;
        let item = at(&at(&catalog.lists, 0)?.items, 0)?;

        assert_eq!(
            item.clone_of.as_ref().map(SoftwareItemName::as_str),
            Some("omitted_parent")
        );
        assert_eq!(at(&item.info, 0)?.value, None);
        assert_eq!(at(&item.shared_features, 0)?.value, None);
        Ok(())
    }

    #[test]
    fn parser_materializes_dtd_defaults_and_parses_dipswitches() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one"><notes>list notes</notes><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="first.bin"/></dataarea><dataarea name="rom" size="2"><rom name="second.bin"/></dataarea><diskarea name="media"><disk name="implicit"/><disk name="explicit" writeable="no"/></diskarea><dipswitch name="Difficulty" tag=":DSW" mask="0x03"><dipvalue name="Easy" value="0x01" default="yes"/><dipvalue name="Hard" value="0x02"/></dipswitch></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert_eq!(catalog.lists[0].notes.as_deref(), Some("list notes"));
        let item = at(&at(&catalog.lists, 0)?.items, 0)?;
        assert_eq!(item.supported, Some(SupportedStatus::Yes));
        let part = at(&item.parts, 0)?;
        let areas = &part.areas;
        assert_eq!(areas.len(), 3);
        assert_eq!(areas[0].name, areas[1].name);
        assert_eq!(areas[0].declared_size, Some(1));
        assert_eq!(areas[1].declared_size, Some(2));
        assert_eq!(areas[0].width, Some(8));
        assert_eq!(areas[0].endianness, Some(Endianness::Little));
        assert_eq!(
            as_rom(at(&areas[0].components, 0)?)?.status,
            Some(DumpStatus::Good)
        );
        assert_eq!(
            as_rom(at(&areas[1].components, 0)?)?
                .name
                .as_ref()
                .map(ComponentName::as_str),
            Some("second.bin")
        );
        assert_eq!(disk(at(&areas[2].components, 0)?)?.writeable, Some(false));
        assert_eq!(disk(at(&areas[2].components, 1)?)?.writeable, Some(false));
        assert_eq!(
            disk(at(&areas[2].components, 0)?)?.status,
            Some(DumpStatus::Good)
        );
        let switch = at(&part.dipswitches, 0)?;
        assert_eq!(switch.name.as_str(), "Difficulty");
        assert_eq!(switch.tag, ":DSW");
        assert_eq!(switch.mask, "0x03");
        assert_eq!(switch.values[0].name, "Easy");
        assert_eq!(switch.values[0].value, "0x01");
        assert!(switch.values[0].is_default);
        assert!(!switch.values[1].is_default);

        let empty_text_fields = br#"<softwarelist name="empty-text"><software name="game"><description/><year/><publisher/></software></softwarelist>"#;
        let empty_text_catalog = SoftwareListCatalog::parse(empty_text_fields)?;
        let empty_text_item = &empty_text_catalog.lists[0].items[0];
        assert_eq!(empty_text_item.description, "");
        assert_eq!(empty_text_item.year, "");
        assert_eq!(empty_text_item.publisher, "");
        Ok(())
    }

    #[test]
    fn parser_uses_mame_number_bases_and_treats_empty_hashes_as_absent() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="010"><rom name="missing.bin" size="010" offset="010" status="nodump" crc="" sha1=""/></dataarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        let area = at(&at(&at(&catalog.lists, 0)?.items, 0)?.parts, 0)?;
        let area = at(&area.areas, 0)?;
        assert_eq!(area.declared_size, Some(8));
        let rom = as_rom(at(&area.components, 0)?)?;
        assert_eq!(rom.size, Some(8));
        assert_eq!(rom.offset, Some(8));
        assert_eq!(rom.crc, None);
        assert_eq!(rom.crc_text.as_deref(), Some(""));
        assert_eq!(rom.sha1, None);
        assert_eq!(rom.sha1_text.as_deref(), Some(""));
        assert_eq!(rom.status, Some(DumpStatus::NoDump));
        assert_eq!(parse_number("0x10")?, 16);
        assert!(parse_number("08").is_err());
        Ok(())
    }

    #[test]
    fn parser_keeps_uninterpretable_source_values_instead_of_rejecting_them() -> crate::Result<()> {
        for size_text in ["", "unparsed", "9223372036854775808"] {
            let xml = format!(
                "<softwarelist name=\"one\"><software name=\"game\"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name=\"cart\" interface=\"cart\"><dataarea name=\"rom\" size=\"{size_text}\"><rom name=\"game.bin\" crc=\"not-hex\"/></dataarea></part></software></softwarelist>"
            );
            let catalog = SoftwareListCatalog::parse(xml.as_bytes())?;
            let area = &catalog.lists[0].items[0].parts[0].areas[0];
            assert_eq!(area.declared_size, None);
            assert_eq!(area.declared_size_text.as_deref(), Some(size_text));
            let rom = as_rom(&area.components[0])?;
            assert_eq!(rom.crc, None);
            assert_eq!(rom.crc_text.as_deref(), Some("not-hex"));
        }
        Ok(())
    }

    #[test]
    fn parser_rejects_external_entities_but_retains_invalid_hash_source_text() -> crate::Result<()>
    {
        let entity = br#"<!DOCTYPE softwarelist [<!ENTITY external SYSTEM "file:///etc/passwd">]><softwarelist name="one"><software name="game"><description>&external;</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert!(matches!(
            SoftwareListCatalog::parse(entity),
            Err(crate::Error::XmlEntityNotAllowed)
        ));

        let invalid_hash = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="16"><rom name="game.bin" crc="not-hex"/></dataarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(invalid_hash)?;
        let rom = as_rom(
            &at(
                &at(&at(&at(&catalog.lists, 0)?.items, 0)?.parts, 0)?.areas,
                0,
            )?
            .components[0],
        )?;
        assert_eq!(rom.crc, None);
        assert_eq!(rom.crc_text.as_deref(), Some("not-hex"));

        let encoded = "<?xml version=\"1.0\" encoding=\"UTF-16\"?><!DOCTYPE softwarelist [<!ENTITY secret \"expanded\">]><softwarelist name=\"one\"><software name=\"game\"><description>&secret;</description><year>2000</year><publisher>Pub</publisher><part name=\"cart\" interface=\"cart\"/></software></softwarelist>";
        let mut utf16 = vec![0xff, 0xfe];
        for unit in encoded.encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        assert!(matches!(
            SoftwareListCatalog::parse(&utf16),
            Err(crate::Error::XmlEntityNotAllowed)
        ));

        let mut no_bom = Vec::new();
        for unit in
            "<!DOCTYPE softwarelist [<!ENTITY secret \"expanded\">]><softwarelist name=\"one\"/>"
                .encode_utf16()
        {
            no_bom.extend_from_slice(&unit.to_le_bytes());
        }
        assert!(matches!(
            SoftwareListCatalog::parse(&no_bom),
            Err(crate::Error::XmlEntityNotAllowed)
        ));
        Ok(())
    }

    #[test]
    fn parser_preserves_envelope_presence_source_order_and_raw_cdata() -> crate::Result<()> {
        let xml = br#"<softwarelists build="0.289"><softwarelist name="one" description="list description"><notes> list notes </notes><software name="game" supported="yes"><description> Game </description><year>2000</year><publisher>Pub</publisher><notes> item notes </notes><info name="language"/><sharedfeat name="compatibility"/><part name="cart" interface="cart"><feature name="board"/><dataarea name="first" size="unparsed" width="8" endianness="little"><rom name="bad.bin" size="unparsed" offset="" crc="bad-crc" sha1="bad-sha1"/><rom name="good.bin" status="good"/></dataarea><diskarea name="media"><disk name="implicit" sha1="bad-disk-sha1"/><disk name="explicit" status="good" writeable="no"/></diskarea><dipswitch name="Mode" tag=":SW" mask="0x03"><dipvalue name="Default" value="0x01"/><dipvalue name="Explicit" value="0x02" default="no"/></dipswitch><dataarea name="empty" size=""/></part></software></softwarelist><softwarelist name="two"><software name="game"><description>G</description><year>2001</year><publisher>P</publisher></software></softwarelist></softwarelists>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert_eq!(catalog.root_kind, SoftwareListRootKind::PluralLists);
        assert_eq!(catalog.root_kind.as_str(), "plural_lists");
        assert_eq!(catalog.build.as_deref(), Some("0.289"));
        assert_eq!(catalog.lists[0].source_order, 0);
        assert_eq!(catalog.lists[1].source_order, 1);
        assert_eq!(catalog.lists[0].text_positions.len(), 1);
        assert_eq!(
            catalog.lists[0].text_positions[0].field,
            SoftwareTextField::Notes
        );
        assert_eq!(catalog.lists[0].text_positions[0].source_order, 0);
        assert_eq!(catalog.lists[0].text_positions[0].field.as_code(), 3);

        let item = &catalog.lists[0].items[0];
        assert_eq!(item.source_order, 1);
        assert!(item.supported_specified);
        assert_eq!(item.description, " Game ");
        assert_eq!(item.notes.as_deref(), Some(" item notes "));
        assert_eq!(
            item.text_positions
                .iter()
                .map(|position| (position.field.as_code(), position.source_order))
                .collect::<Vec<_>>(),
            [(0, 0), (1, 1), (2, 2), (3, 3)]
        );
        let part = &item.parts[0];
        assert_eq!(part.source_order, 6);
        assert_eq!(part.features[0].source_order, 0);
        assert_eq!(item.info[0].source_order, 4);
        assert_eq!(item.shared_features[0].source_order, 5);
        assert_eq!(
            part.areas
                .iter()
                .map(|area| area.source_order)
                .collect::<Vec<_>>(),
            [1, 2, 4]
        );
        assert_eq!(part.dipswitches[0].source_order, 3);
        assert_eq!(part.dipswitches[0].values[0].source_order, 0);
        assert!(!part.dipswitches[0].values[0].default_specified);
        assert!(part.dipswitches[0].values[1].default_specified);

        let first_area = &part.areas[0];
        assert_eq!(first_area.declared_size, None);
        assert_eq!(first_area.declared_size_text.as_deref(), Some("unparsed"));
        assert!(first_area.width_specified);
        assert!(first_area.endianness_specified);
        let first_rom = as_rom(&first_area.components[0])?;
        assert_eq!(first_rom.source_order, 0);
        assert_eq!(first_rom.size, None);
        assert_eq!(first_rom.size_text.as_deref(), Some("unparsed"));
        assert_eq!(first_rom.offset, None);
        assert_eq!(first_rom.offset_text.as_deref(), Some(""));
        assert_eq!(first_rom.crc, None);
        assert_eq!(first_rom.crc_text.as_deref(), Some("bad-crc"));
        assert_eq!(first_rom.sha1, None);
        assert_eq!(first_rom.sha1_text.as_deref(), Some("bad-sha1"));
        assert!(!first_rom.status_specified);
        assert!(as_rom(&first_area.components[1])?.status_specified);

        let implicit_disk = disk(&part.areas[1].components[0])?;
        assert_eq!(implicit_disk.source_order, 0);
        assert_eq!(implicit_disk.sha1_text.as_deref(), Some("bad-disk-sha1"));
        assert_eq!(implicit_disk.requirement.expected_sha1(), None);
        assert!(!implicit_disk.status_specified);
        assert!(!implicit_disk.writeable_specified);
        let explicit_disk = disk(&part.areas[1].components[1])?;
        assert!(explicit_disk.status_specified);
        assert!(explicit_disk.writeable_specified);
        assert_eq!(part.areas[2].declared_size_text.as_deref(), Some(""));
        assert_eq!(part.areas[2].declared_size, None);
        assert!(!part.areas[2].width_specified);
        assert!(!part.areas[2].endianness_specified);

        let single = SoftwareListCatalog::parse(
            br#"<softwarelist name="single"><software name="game"><description>D</description><year>2000</year><publisher>P</publisher></software></softwarelist>"#,
        )?;
        assert_eq!(single.root_kind, SoftwareListRootKind::SingleList);
        assert_eq!(single.root_kind.as_str(), "single_list");
        assert_eq!(single.build, None);
        Ok(())
    }

    #[test]
    fn width_uses_the_declared_enum_not_numeric_equivalence() {
        for width in ["08", "+8", " 8", "0x8"] {
            let xml = format!(
                r#"<softwarelist name="one"><software name="game"><description>G</description><year>2000</year><publisher>P</publisher><part name="cart" interface="cart"><dataarea name="rom" size="8" width="{width}"/></part></software></softwarelist>"#
            );
            assert!(
                matches!(
                    SoftwareListCatalog::parse(xml.as_bytes()),
                    Err(crate::Error::CatalogParse { .. })
                ),
                "accepted non-enum width {width:?}"
            );
        }
    }

    #[test]
    fn invalid_enumerated_values_report_the_record_and_source_location() {
        let source =
            String::from_utf8_lossy(include_bytes!("../fixtures/catalog/mame/software-list.xml"));
        for (original, replacement, expected_kind, expected_record) in [
            (
                "supported=\"partial\"",
                "supported=\"unsupported\"",
                "software",
                "demo_cart:demo_game",
            ),
            (
                "status=\"nodump\"",
                "status=\"unknown\"",
                "rom",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "loadflag=\"continue\"",
                "loadflag=\"unknown\"",
                "rom",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "writeable=\"yes\"",
                "writeable=\"unknown\"",
                "disk",
                "demo_cart:demo_game:cart:disk:media",
            ),
            (
                "width=\"16\"",
                "width=\"7\"",
                "dataarea",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "endianness=\"big\"",
                "endianness=\"middle\"",
                "dataarea",
                "demo_cart:demo_game:cart:data:program",
            ),
        ] {
            let xml = source.replacen(original, replacement, 1);
            let result = SoftwareListCatalog::parse(xml.as_bytes());
            assert!(matches!(
                result,
                Err(crate::Error::CatalogParse {
                    record_kind: Some(kind),
                    record_name: Some(record),
                    line: Some(line),
                    column: Some(column),
                    ..
                }) if kind == expected_kind && record == expected_record && line > 0 && column > 0
            ));
        }
    }
}
