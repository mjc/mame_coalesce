//! Grammar validator for the pinned Logiqx DTD 1.5 interpretation.

use crate::xml_reader::{Element, ElementContent, ElementContentKind};

const DTD_REVISION: &str = "1.5";
const DTD_COMMIT: &str = "1575f8da6706e159b51e3bb8b511f546909927cc";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootState {
    HeaderAllowed,
    GameRequired,
    Games,
}

/// Effects of the document's external declaration on its standalone promise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeclarationPolicy {
    Ordinary,
    StandaloneExternal,
}

impl DeclarationPolicy {
    pub(super) fn validate_container_text(self, text: &str, owner: &str) -> crate::Result<()> {
        if !xml_whitespace(text) {
            return Err(invalid(format!(
                "character data is not allowed in element-only <{owner}>"
            )));
        }
        if self == Self::StandaloneExternal && !text.is_empty() {
            return Err(invalid(format!(
                "standalone document depends on external element-content whitespace in <{owner}>"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum HeaderChild {
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
    ClrMamePro,
    RomCenter,
}

impl HeaderChild {
    fn parse(name: &str) -> Option<Self> {
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
            "clrmamepro" => Some(Self::ClrMamePro),
            "romcenter" => Some(Self::RomCenter),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum GameChild {
    Comment,
    Description,
    Year,
    Manufacturer,
    Release,
    BiosSet,
    Rom,
    Disk,
    Sample,
    Archive,
}

impl GameChild {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "comment" => Some(Self::Comment),
            "description" => Some(Self::Description),
            "year" => Some(Self::Year),
            "manufacturer" => Some(Self::Manufacturer),
            "release" => Some(Self::Release),
            "biosset" => Some(Self::BiosSet),
            "rom" => Some(Self::Rom),
            "disk" => Some(Self::Disk),
            "sample" => Some(Self::Sample),
            "archive" => Some(Self::Archive),
            _ => None,
        }
    }

    const fn repeatable(self) -> bool {
        matches!(
            self,
            Self::Comment
                | Self::Release
                | Self::BiosSet
                | Self::Rom
                | Self::Disk
                | Self::Sample
                | Self::Archive
        )
    }
}

#[derive(Debug)]
pub(super) struct Validator {
    root_state: RootState,
    declaration_policy: DeclarationPolicy,
}

impl Validator {
    pub(super) fn new(
        root: &mut Element,
        declaration_policy: DeclarationPolicy,
    ) -> crate::Result<Self> {
        if root.name != "datafile" {
            return Err(invalid(format!("unexpected root element <{}>", root.name)));
        }
        normalize_enumerations(root, declaration_policy)?;
        attributes(root, DATAFILE_ATTRIBUTES)?;
        Ok(Self {
            root_state: RootState::HeaderAllowed,
            declaration_policy,
        })
    }

    pub(super) fn accept_root_child(&mut self, child: &mut Element) -> crate::Result<()> {
        normalize_enumerations(child, self.declaration_policy)?;
        match child.name.as_str() {
            "header" if self.root_state == RootState::HeaderAllowed => {
                validate_header(child, self.declaration_policy)?;
                self.root_state = RootState::GameRequired;
            }
            "game" => {
                validate_game(child, self.declaration_policy)?;
                self.root_state = RootState::Games;
            }
            "header" => return Err(invalid("<header> must precede every <game>")),
            name => return Err(invalid(format!("undeclared <{name}> in <datafile>"))),
        }
        Ok(())
    }

    pub(super) fn finish(self) -> crate::Result<()> {
        if self.root_state != RootState::Games {
            return Err(invalid("<datafile> requires at least one <game>"));
        }
        Ok(())
    }
}

fn validate_header(header: &Element, policy: DeclarationPolicy) -> crate::Result<()> {
    validate_element_only(header, "header", policy)?;
    attributes(header, &[])?;
    let mut previous = None;
    let mut seen = Vec::new();
    for child in header.children() {
        let field = HeaderChild::parse(&child.name)
            .ok_or_else(|| invalid(format!("undeclared <{}> in <header>", child.name)))?;
        ordered_child(&mut previous, field, &mut seen, false, "header")?;
        match field {
            HeaderChild::ClrMamePro => validate_empty(child, CLRMAMEPRO_ATTRIBUTES)?,
            HeaderChild::RomCenter => validate_empty(child, ROMCENTER_ATTRIBUTES)?,
            _ => validate_pcdata(child)?,
        }
    }
    for required in [
        HeaderChild::Name,
        HeaderChild::Description,
        HeaderChild::Version,
        HeaderChild::Author,
    ] {
        if !seen.contains(&required) {
            return Err(invalid(format!(
                "<header> is missing <{}>",
                header_name(required)
            )));
        }
    }
    Ok(())
}

fn validate_game(game: &Element, policy: DeclarationPolicy) -> crate::Result<()> {
    validate_element_only(game, "game", policy)?;
    attributes(game, GAME_ATTRIBUTES)?;
    let mut previous = None;
    let mut seen = Vec::new();
    for child in game.children() {
        let field = GameChild::parse(&child.name)
            .ok_or_else(|| invalid(format!("undeclared <{}> in <game>", child.name)))?;
        ordered_child(&mut previous, field, &mut seen, field.repeatable(), "game")?;
        match field {
            GameChild::Comment
            | GameChild::Description
            | GameChild::Year
            | GameChild::Manufacturer => {
                validate_pcdata(child)?;
            }
            GameChild::Release => validate_empty(child, RELEASE_ATTRIBUTES)?,
            GameChild::BiosSet => validate_empty(child, BIOS_ATTRIBUTES)?,
            GameChild::Rom => validate_empty(child, ROM_ATTRIBUTES)?,
            GameChild::Disk => validate_empty(child, DISK_ATTRIBUTES)?,
            GameChild::Sample | GameChild::Archive => validate_empty(child, NAME_ATTRIBUTES)?,
        }
    }
    if !seen.contains(&GameChild::Description) {
        return Err(invalid("<game> is missing required <description>"));
    }
    Ok(())
}

fn validate_element_only(
    element: &Element,
    name: &str,
    policy: DeclarationPolicy,
) -> crate::Result<()> {
    if element.content_kind == ElementContentKind::CharacterData {
        return Err(invalid(format!(
            "character data is not allowed in element-only <{name}>"
        )));
    }
    for content in &element.content {
        if let ElementContent::Text(text) = content {
            policy.validate_container_text(text, name)?;
        }
    }
    Ok(())
}

fn ordered_child<T: Copy + Ord + Eq>(
    previous: &mut Option<T>,
    current: T,
    seen: &mut Vec<T>,
    repeatable: bool,
    parent: &str,
) -> crate::Result<()> {
    if previous.is_some_and(|previous| current < previous) {
        return Err(invalid(format!(
            "child <{parent}> content is out of DTD order"
        )));
    }
    if !repeatable && seen.contains(&current) {
        return Err(invalid(format!("duplicate child in <{parent}>")));
    }
    if !seen.contains(&current) {
        seen.push(current);
    }
    *previous = Some(current);
    Ok(())
}

fn validate_pcdata(element: &Element) -> crate::Result<()> {
    attributes(element, &[])?;
    if element
        .content
        .iter()
        .any(|content| matches!(content, ElementContent::Element(_)))
    {
        return Err(invalid(format!(
            "child markup is not allowed in <{}>",
            element.name
        )));
    }
    Ok(())
}

fn validate_empty(
    element: &Element,
    allowed: &[(&str, bool, Option<&[&str]>)],
) -> crate::Result<()> {
    attributes(element, allowed)?;
    if element.content_kind != ElementContentKind::Empty {
        return Err(invalid(format!(
            "content is not allowed in EMPTY <{}>",
            element.name
        )));
    }
    Ok(())
}

type AttributeRule<'a> = (&'a str, bool, Option<&'a [&'a str]>);

fn attributes(element: &Element, allowed: &[AttributeRule<'_>]) -> crate::Result<()> {
    if element.has_namespace_declarations {
        return Err(invalid(format!(
            "namespace declarations are not allowed on <{}>",
            element.name
        )));
    }
    for (name, value) in element.attributes.declared_iter() {
        let rule = allowed
            .iter()
            .find(|(candidate, _, _)| *candidate == name)
            .ok_or_else(|| {
                invalid(format!(
                    "undeclared attribute {name:?} on <{}>",
                    element.name
                ))
            })?;
        if let Some(values) = rule.2
            && !values.contains(&value.value.as_str())
        {
            return Err(invalid(format!(
                "invalid {name} enumeration value {value:?}"
            )));
        }
    }
    for (name, required, _) in allowed {
        if *required && !element.attributes.contains_key(name) {
            return Err(invalid(format!(
                "missing required {name:?} on <{}>",
                element.name
            )));
        }
    }
    Ok(())
}

/// Canonicalize the 15 enumeration values in their existing declarations.
/// Source positions remain intact, and CDATA attributes are left untouched.
fn normalize_enumerations(element: &mut Element, policy: DeclarationPolicy) -> crate::Result<()> {
    // All 15 enumerated attributes in the pinned grammar also have defaults.
    for (field, _, enumeration) in attribute_rules(&element.name) {
        if enumeration.is_none() {
            continue;
        }
        if let Some(value) = element
            .attributes
            .get(field)
            .map(|value| collapse_xml_spaces(value))
        {
            if policy == DeclarationPolicy::StandaloneExternal
                && element.attributes.get(field) != Some(&value)
            {
                return Err(invalid(format!(
                    "standalone document depends on external normalization of {field:?} on <{}>",
                    element.name
                )));
            }
            let _ = element.attributes.update_value(field, value);
        } else if policy == DeclarationPolicy::StandaloneExternal {
            return Err(invalid(format!(
                "standalone document depends on external default for {field:?} on <{}>",
                element.name
            )));
        }
    }
    for child in &mut element.content {
        if let ElementContent::Element(child) = child {
            normalize_enumerations(child, policy)?;
        }
    }
    Ok(())
}

fn attribute_rules(name: &str) -> &'static [AttributeRule<'static>] {
    match name {
        "datafile" => DATAFILE_ATTRIBUTES,
        "game" => GAME_ATTRIBUTES,
        "clrmamepro" => CLRMAMEPRO_ATTRIBUTES,
        "romcenter" => ROMCENTER_ATTRIBUTES,
        "release" => RELEASE_ATTRIBUTES,
        "biosset" => BIOS_ATTRIBUTES,
        "rom" => ROM_ATTRIBUTES,
        "disk" => DISK_ATTRIBUTES,
        "sample" | "archive" => NAME_ATTRIBUTES,
        _ => &[],
    }
}

fn collapse_xml_spaces(value: &str) -> String {
    let mut collapsed = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if character == ' ' {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
                pending_space = false;
            }
            collapsed.push(character);
        }
    }
    collapsed
}

pub(super) fn xml_whitespace(text: &str) -> bool {
    text.chars()
        .all(|character| matches!(character, ' ' | '\t' | '\r' | '\n'))
}

fn invalid(message: impl Into<String>) -> crate::Error {
    crate::Error::XmlValidation(format!(
        "Logiqx DTD {DTD_REVISION} ({DTD_COMMIT}): {}",
        message.into()
    ))
}

const fn header_name(child: HeaderChild) -> &'static str {
    match child {
        HeaderChild::Name => "name",
        HeaderChild::Description => "description",
        HeaderChild::Category => "category",
        HeaderChild::Version => "version",
        HeaderChild::Date => "date",
        HeaderChild::Author => "author",
        HeaderChild::Email => "email",
        HeaderChild::Homepage => "homepage",
        HeaderChild::Url => "url",
        HeaderChild::Comment => "comment",
        HeaderChild::ClrMamePro => "clrmamepro",
        HeaderChild::RomCenter => "romcenter",
    }
}

const GAME_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("name", true, None),
    ("sourcefile", false, None),
    ("isbios", false, Some(&["yes", "no"])),
    ("cloneof", false, None),
    ("romof", false, None),
    ("sampleof", false, None),
    ("board", false, None),
    ("rebuildto", false, None),
];
const DATAFILE_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("build", false, None),
    ("debug", false, Some(&["yes", "no"])),
];
const RELEASE_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("name", true, None),
    ("region", true, None),
    ("language", false, None),
    ("date", false, None),
    ("default", false, Some(&["yes", "no"])),
];
const BIOS_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("name", true, None),
    ("description", true, None),
    ("default", false, Some(&["yes", "no"])),
];
const ROM_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("name", true, None),
    ("size", true, None),
    ("crc", false, None),
    ("sha1", false, None),
    ("md5", false, None),
    ("merge", false, None),
    (
        "status",
        false,
        Some(&["baddump", "nodump", "good", "verified"]),
    ),
    ("date", false, None),
];
const DISK_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("name", true, None),
    ("sha1", false, None),
    ("md5", false, None),
    ("merge", false, None),
    (
        "status",
        false,
        Some(&["baddump", "nodump", "good", "verified"]),
    ),
];
const NAME_ATTRIBUTES: &[AttributeRule<'static>] = &[("name", true, None)];
const CLRMAMEPRO_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("header", false, None),
    ("forcemerging", false, Some(&["none", "split", "full"])),
    (
        "forcenodump",
        false,
        Some(&["obsolete", "required", "ignore"]),
    ),
    ("forcepacking", false, Some(&["zip", "unzip"])),
];
const ROMCENTER_ATTRIBUTES: &[AttributeRule<'static>] = &[
    ("plugin", false, None),
    ("rommode", false, Some(&["merged", "split", "unmerged"])),
    ("biosmode", false, Some(&["merged", "split", "unmerged"])),
    ("samplemode", false, Some(&["merged", "unmerged"])),
    ("lockrommode", false, Some(&["yes", "no"])),
    ("lockbiosmode", false, Some(&["yes", "no"])),
    ("locksamplemode", false, Some(&["yes", "no"])),
];
