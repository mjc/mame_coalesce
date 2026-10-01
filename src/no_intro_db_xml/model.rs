use crate::{diagnostics::SourceExcerpt, logiqx::RecordLocation, xml_reader::DeclaredText};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Selects an observed-compatible No-Intro database-export interpretation.
pub enum NoIntroDatabaseMode {
    /// Parse observed XML without recovery of XML-forbidden characters.
    ObservedCompatible,
    /// Replace only decoded U+0000 scalars in the parse view and retain a warning per scalar.
    NullRecoveryCompatible,
}

impl NoIntroDatabaseMode {
    /// Return the canonical catalog-format identifier for this interpretation.
    #[must_use]
    pub const fn format(self) -> &'static str {
        match self {
            Self::ObservedCompatible => "no-intro-database-xml-compatible",
            Self::NullRecoveryCompatible => "no-intro-database-xml-nul-compatible",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Describes which supported top-level XML framing was used.
pub enum EnvelopeKind {
    /// A single `<datafile>` root, optionally containing `<header>`.
    SingleDatafile,
    /// A sibling `<header>` root followed by a sibling `<datafile>` root.
    SiblingHeaderDatafile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One decoded U+0000 replaced in the parser's in-memory XML view.
pub struct RecoveryWarning {
    /// The original decoded scalar.
    pub replaced: char,
    /// The scalar substituted in the in-memory parse view.
    pub replacement: char,
    /// One-based line and Unicode-scalar column in the decoded original.
    pub location: RecordLocation,
    /// Exact encoded bytes for this NUL, in the retained or gzip-decoded view.
    pub excerpt: Option<SourceExcerpt>,
}

#[derive(Debug, PartialEq, Eq)]
/// Document-level parse facts passed to the caller before games are consumed.
pub struct NoIntroDatabaseDocument {
    /// The observed root framing.
    pub envelope: EnvelopeKind,
    /// The selected interpretation, including any NUL recovery policy.
    pub mode: NoIntroDatabaseMode,
    /// Location of the `<datafile>` root.
    pub location: RecordLocation,
    /// Optional header with repeated fields retained in source order.
    pub header: Option<DatabaseHeader>,
}

#[derive(Debug, PartialEq, Eq)]
/// Optional export header and its ordered, repeatable text fields.
pub struct DatabaseHeader {
    /// Location of the `<header>` element.
    pub location: RecordLocation,
    /// Header children in source order, including repeated and empty values.
    pub fields: Vec<HeaderField>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Closed set of recognized database-export header child names.
pub enum HeaderFieldKind {
    /// `<author>` text.
    Author,
    /// `<piracy>` text.
    Piracy,
    /// `<trademarks>` text.
    Trademarks,
    /// `<url>` text.
    Url,
    /// `<version>` text.
    Version,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One present header field and its source text/location.
pub struct HeaderField {
    /// Recognized literal element name.
    pub kind: HeaderFieldKind,
    /// Decoded text, child ordinal, and element location.
    pub value: DeclaredText,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One catalog game.
///
/// Repeated archives and mixed source/release children stay local to it.
pub struct DatabaseGame {
    /// Zero-based position among game elements in the datafile.
    pub list_order: usize,
    /// Location of the `<game>` element.
    pub location: RecordLocation,
    /// Required `name` attribute, including a present empty spelling.
    pub name: DeclaredText,
    /// Repeatable game-owned archive descriptions.
    pub archives: Vec<ArchiveDescription>,
    /// Preserves interleaving between dump histories and releases.
    pub source_or_release: Vec<SourceOrRelease>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One repeatable `<archive>` declaration with literal source attributes.
///
/// Each optional attribute field uses its identical XML name; `None` means
/// absent and `Some` with empty text means present-empty. Clone and merge
/// declarations have distinct fields; no target is resolved.
pub struct ArchiveDescription {
    /// Raw ordinal among the game's child elements.
    pub source_order: usize,
    /// Location of the `<archive>` element.
    pub location: RecordLocation,
    /// Optional source attribute `additional`.
    pub additional: Option<DeclaredText>,
    pub adult: Option<DeclaredText>,
    pub aftermarket: Option<DeclaredText>,
    pub alt: Option<DeclaredText>,
    pub bios: Option<DeclaredText>,
    pub categories: Option<DeclaredText>,
    pub complete: Option<DeclaredText>,
    pub dat: Option<DeclaredText>,
    pub datter_note: Option<DeclaredText>,
    pub description: Option<DeclaredText>,
    pub devstatus: Option<DeclaredText>,
    pub gameid1: Option<DeclaredText>,
    pub gameid2: Option<DeclaredText>,
    pub langchecked: Option<DeclaredText>,
    pub languages: Option<DeclaredText>,
    pub licensed: Option<DeclaredText>,
    pub listed: Option<DeclaredText>,
    pub mergename: Option<DeclaredText>,
    pub name: Option<DeclaredText>,
    pub name_alt: Option<DeclaredText>,
    pub number: Option<DeclaredText>,
    pub physical: Option<DeclaredText>,
    pub region: Option<DeclaredText>,
    pub regparent: Option<DeclaredText>,
    pub showlang: Option<DeclaredText>,
    pub special1: Option<DeclaredText>,
    pub special2: Option<DeclaredText>,
    pub sticky_note: Option<DeclaredText>,
    pub version1: Option<DeclaredText>,
    pub version2: Option<DeclaredText>,
    pub clone: Option<ArchiveClone>,
    pub mergeof: Option<DeclaredText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Raw archive clone declaration, distinguishing only the literal `P` marker.
pub enum ArchiveClone {
    /// The exact `P` marker value.
    ParentMarker(DeclaredText),
    /// Any other literal clone value, without inferring a target identity.
    OtherValue(DeclaredText),
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A source or release in the original game-child order.
pub enum SourceOrRelease {
    /// Dump-history source owner.
    Source(Box<DumpSource>),
    /// Separately modeled release owner.
    Release(Box<DatabaseRelease>),
}

impl SourceOrRelease {
    /// Return the raw sibling ordinal within the game.
    #[must_use]
    pub const fn source_order(&self) -> usize {
        match self {
            Self::Source(value) => value.source_order,
            Self::Release(value) => value.source_order,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One dump-history owner with its own optional details/serials and repeatable files.
pub struct DumpSource {
    /// Raw ordinal among the game children.
    pub source_order: usize,
    /// Location of the `<source>` element.
    pub location: RecordLocation,
    /// Optional singleton source details element; `Some` preserves an empty element.
    pub details: Option<SourceDetails>,
    /// Optional singleton source serials element; `Some` preserves an empty element.
    pub serials: Option<SourceSerials>,
    /// Repeatable source-owned file declarations in source order.
    pub files: Vec<SourceFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Source-specific attributes on one `<details>` element.
/// Every optional field is the literal XML attribute; absence differs from empty text.
pub struct SourceDetails {
    pub source_order: usize,
    pub location: RecordLocation,
    /// Exclusive position immediately after the complete `<details>` opening tag.
    pub opening_end: RecordLocation,
    pub comment1: Option<DeclaredText>,
    pub comment2: Option<DeclaredText>,
    pub d_date: Option<DeclaredText>,
    pub d_date_info: Option<DeclaredText>,
    pub dumper: Option<DeclaredText>,
    pub id: Option<DeclaredText>,
    pub link1: Option<DeclaredText>,
    pub link2: Option<DeclaredText>,
    pub link3: Option<DeclaredText>,
    pub media_title: Option<DeclaredText>,
    pub nodump: Option<DeclaredText>,
    pub origin: Option<DeclaredText>,
    pub originalformat: Option<DeclaredText>,
    pub project: Option<DeclaredText>,
    pub r_date: Option<DeclaredText>,
    pub r_date_info: Option<DeclaredText>,
    pub region: Option<DeclaredText>,
    pub rominfo: Option<DeclaredText>,
    pub section: Option<DeclaredText>,
    pub tool: Option<DeclaredText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Source-specific serial attributes on one `<serials>` element. Optional
/// fields map directly to the same-named attributes and preserve presence.
pub struct SourceSerials {
    pub source_order: usize,
    pub location: RecordLocation,
    pub box_barcode: Option<DeclaredText>,
    pub box_serial: Option<DeclaredText>,
    pub chip_serial: Option<DeclaredText>,
    pub digital_serial1: Option<DeclaredText>,
    pub digital_serial2: Option<DeclaredText>,
    pub lockout_serial: Option<DeclaredText>,
    pub media_serial1: Option<DeclaredText>,
    pub media_serial2: Option<DeclaredText>,
    pub media_serial3: Option<DeclaredText>,
    pub mediastamp: Option<DeclaredText>,
    pub pcb_serial: Option<DeclaredText>,
    pub romchip_serial1: Option<DeclaredText>,
    pub romchip_serial2: Option<DeclaredText>,
    pub savechip_serial: Option<DeclaredText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One release-history owner with its distinct details, serials, and repeatable files.
pub struct DatabaseRelease {
    pub source_order: usize,
    pub location: RecordLocation,
    pub details: Option<ReleaseDetails>,
    pub serials: Option<ReleaseSerials>,
    pub files: Vec<ReleaseFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Release-specific `<details>` attributes. Optional fields map directly to
/// the same-named attributes, and the NFO CRC aliases remain separate.
pub struct ReleaseDetails {
    pub source_order: usize,
    pub location: RecordLocation,
    /// Exclusive position immediately after the complete `<details>` opening tag.
    pub opening_end: RecordLocation,
    pub archivename: Option<DeclaredText>,
    pub category: Option<DeclaredText>,
    pub comment: Option<DeclaredText>,
    pub date: Option<DeclaredText>,
    pub dirname: Option<DeclaredText>,
    pub group: Option<DeclaredText>,
    pub id: Option<DeclaredText>,
    pub nfo_crc32: Option<DatabaseDigest>,
    pub nfo_size: Option<DeclaredText>,
    pub nfocrc: Option<DatabaseDigest>,
    pub nfoname: Option<DeclaredText>,
    pub nfosize: Option<DeclaredText>,
    pub origin: Option<DeclaredText>,
    pub originalformat: Option<DeclaredText>,
    pub region: Option<DeclaredText>,
    pub rominfo: Option<DeclaredText>,
    pub tool: Option<DeclaredText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Release-specific serial attributes; this set is narrower than source
/// serials. Optional fields map directly to the same-named attributes.
pub struct ReleaseSerials {
    pub source_order: usize,
    pub location: RecordLocation,
    pub box_barcode: Option<DeclaredText>,
    pub box_serial: Option<DeclaredText>,
    pub media_serial1: Option<DeclaredText>,
    pub mediastamp: Option<DeclaredText>,
    pub pcb_serial: Option<DeclaredText>,
    pub romchip_serial1: Option<DeclaredText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One file declaration owned by a dump source. Optional fields map directly
/// to same-named attributes; the origin digest and size remain separate facts.
pub struct SourceFile {
    pub source_order: usize,
    pub location: RecordLocation,
    pub bad: Option<DeclaredText>,
    pub crc32: Option<DatabaseDigest>,
    pub date: Option<DeclaredText>,
    pub extension: Option<DeclaredText>,
    pub filter: Option<DeclaredText>,
    pub forcename: Option<DeclaredText>,
    pub forcescenename: Option<DeclaredText>,
    pub format: Option<DeclaredText>,
    pub header: Option<DeclaredText>,
    pub id: Option<DeclaredText>,
    pub item: Option<DeclaredText>,
    pub md5: Option<DatabaseDigest>,
    pub mia: Option<DeclaredText>,
    pub note: Option<DeclaredText>,
    pub origin_sha256: Option<DatabaseDigest>,
    pub origin_size: Option<DeclaredText>,
    pub serial: Option<DeclaredText>,
    pub sha1: Option<DatabaseDigest>,
    pub sha256: Option<DatabaseDigest>,
    pub size: Option<DeclaredText>,
    pub unique: Option<DeclaredText>,
    pub update_type: Option<DeclaredText>,
    pub version: Option<DeclaredText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One file declaration owned by a release. Optional fields map directly to
/// same-named attributes and remain distinct from source-file fields.
pub struct ReleaseFile {
    pub source_order: usize,
    pub location: RecordLocation,
    pub bad: Option<DeclaredText>,
    pub crc32: Option<DatabaseDigest>,
    pub extension: Option<DeclaredText>,
    pub forcename: Option<DeclaredText>,
    pub forcescenename: Option<DeclaredText>,
    pub format: Option<DeclaredText>,
    pub header: Option<DeclaredText>,
    pub id: Option<DeclaredText>,
    pub item: Option<DeclaredText>,
    pub md5: Option<DatabaseDigest>,
    pub note: Option<DeclaredText>,
    pub serial: Option<DeclaredText>,
    pub sha1: Option<DatabaseDigest>,
    pub sha256: Option<DatabaseDigest>,
    pub size: Option<DeclaredText>,
    pub update_type: Option<DeclaredText>,
    pub version: Option<DeclaredText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A present digest literal and its optional decoded hexadecimal bytes.
/// `None` bytes indicate an invalid literal; the source spelling is always retained.
pub struct DatabaseDigest {
    pub source: DeclaredText,
    /// Decoded digest bytes when the literal is valid hex; otherwise `None`.
    pub value: Option<Vec<u8>>,
}
