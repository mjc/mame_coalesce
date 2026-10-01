use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::logiqx::RecordLocation;

const MAX_TOKENS: usize = 1_000_000;
const MAX_FORM_DEPTH: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub version: Option<String>,
    pub header: Option<Header>,
    pub sets: Vec<Set>,
    pub extensions: Vec<Extension>,
    pub comments: Vec<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub source_name: String,
    pub source_order: usize,
    pub name: Option<FieldValue>,
    pub description: Option<FieldValue>,
    pub version: Option<FieldValue>,
    pub date: Option<FieldValue>,
    pub author: Option<FieldValue>,
    pub email: Option<FieldValue>,
    pub homepage: Option<FieldValue>,
    pub url: Option<FieldValue>,
    pub comment: Option<FieldValue>,
    pub category: Option<FieldValue>,
    pub directives: HeaderDirectives,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderDirectives {
    pub header: Option<FieldValue>,
    pub forcemerging: Option<FieldValue>,
    pub forcezipping: Option<FieldValue>,
    pub forcepacking: Option<FieldValue>,
    pub forcenodump: Option<FieldValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldValue {
    pub source_name: String,
    pub value: String,
    pub quoted: bool,
    pub order: usize,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SetFacts {
    pub source_block: String,
    pub document_order: usize,
    pub name: Option<FieldValue>,
    pub cloneof: Option<FieldValue>,
    pub description: Option<FieldValue>,
    pub year: Option<FieldValue>,
    pub region: Option<FieldValue>,
    pub release_year_text: Option<FieldValue>,
    pub release_month_text: Option<FieldValue>,
    pub release_day_text: Option<FieldValue>,
    pub manufacturer: Option<FieldValue>,
    pub rebuildto: Option<FieldValue>,
    pub sampleof: Option<FieldValue>,
    pub serial: Option<FieldValue>,
    pub samples: Vec<FieldValue>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetFacts {
    pub set_order: usize,
    pub name: Option<FieldValue>,
    pub size: Option<FieldValue>,
    pub crc: Option<FieldValue>,
    pub crc32: Option<FieldValue>,
    pub md5: Option<FieldValue>,
    pub sha1: Option<FieldValue>,
    pub merge: Option<FieldValue>,
    pub date: Option<FieldValue>,
    pub serial: Option<FieldValue>,
    pub status_field: Option<FieldValue>,
    pub nodump: Option<FlagValue>,
    pub baddump: Option<FlagValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagValue {
    pub source_name: String,
    pub order: usize,
    pub location: RecordLocation,
}

impl AssetFacts {
    pub(crate) fn has_conflicting_declarations(&self) -> bool {
        let conflicting_crc = self
            .crc
            .as_ref()
            .zip(self.crc32.as_ref())
            .is_some_and(|(crc, crc32)| !crc.value.eq_ignore_ascii_case(&crc32.value));
        let conflicting_dump = self.nodump.is_some() && self.baddump.is_some()
            || self.status_field.is_some() && (self.nodump.is_some() || self.baddump.is_some());

        conflicting_crc || conflicting_dump
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Set {
    pub name: String,
    pub parent: Option<String>,
    pub metadata: Value,
    pub location: RecordLocation,
    pub assets: Vec<Asset>,
    pub extensions: Vec<Extension>,
    pub native: SetFacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub merge: Option<String>,
    pub status: Option<String>,
    pub location: RecordLocation,
    pub metadata: Value,
    pub extensions: Vec<Extension>,
    pub native: AssetFacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    pub record_kind: String,
    pub record_name: Option<String>,
    pub field_name: String,
    pub value: Value,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub text: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TokenKind {
    Word,
    Quoted(String),
    LeftParen,
    RightParen,
    Comment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token<'a> {
    kind: TokenKind,
    raw: &'a str,
    start: usize,
    end: usize,
    location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Field<'a> {
    key: Token<'a>,
    value: Token<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FormItem<'a> {
    Field(Field<'a>),
    Flag(Token<'a>),
    Form(Form<'a>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Form<'a> {
    tag: Token<'a>,
    items: Vec<FormItem<'a>>,
    raw: &'a str,
}

struct Lexer<'a> {
    input: &'a str,
    offset: usize,
    line: i64,
    column: i64,
}

impl Catalog {
    pub(crate) fn parse(bytes: &[u8]) -> crate::Result<Self> {
        let input = std::str::from_utf8(bytes).map_err(|error| {
            parse_error(
                format!("DAT is not valid UTF-8: {error}"),
                "document",
                None,
                None,
            )
        })?;
        let input = input.strip_prefix('\u{feff}').unwrap_or(input);
        let tokens = Lexer::new(input).tokenize()?;
        let mut parser = Parser::new(input, tokens);
        let forms = parser.parse_document()?;
        let mut headers = forms
            .iter()
            .enumerate()
            .filter(|(_, form)| form.tag.word_eq("clrmamepro"));
        let header = headers.next();
        if headers.next().is_some() {
            return Err(parse_error(
                "multiple clrmamepro headers are ambiguous",
                "document",
                None,
                header.map(|(_, form)| form.tag.location),
            ));
        }
        let native_header = header
            .map(|(order, form)| parse_header(form, order))
            .transpose()?;
        let version = native_header
            .as_ref()
            .and_then(|header| header.version.as_ref())
            .map(|field| field.value.clone());

        let mut sets = Vec::new();
        let comments = parser
            .comments
            .into_iter()
            .map(|token| Comment {
                text: token.raw.to_owned(),
                location: token.location,
            })
            .collect::<Vec<_>>();
        let mut extensions = header
            .map(|(_, form)| header_extensions(form))
            .unwrap_or_default();
        for (document_order, form) in forms.iter().enumerate() {
            if form.tag.word_eq("game") || form.tag.word_eq("set") {
                sets.push(parse_set(form, document_order)?);
            } else if !form.tag.word_eq("clrmamepro") {
                extensions.push(Extension {
                    record_kind: "document".into(),
                    record_name: None,
                    field_name: format!("form:{}", form.tag.value()),
                    value: json!({"raw_tokens": form.raw}),
                    location: form.tag.location,
                });
            }
        }
        if sets.is_empty() {
            return Err(parse_error(
                "DAT contains no set/game records",
                "document",
                None,
                None,
            ));
        }
        Ok(Self {
            version,
            header: native_header,
            sets,
            extensions,
            comments,
        })
    }
}

fn header_extensions(form: &Form<'_>) -> Vec<Extension> {
    let known_fields = [
        "name",
        "description",
        "version",
        "date",
        "author",
        "email",
        "homepage",
        "url",
        "comment",
        "category",
        "header",
        "forcemerging",
        "forcezipping",
        "forcepacking",
        "forcenodump",
    ];
    form.items
        .iter()
        .filter_map(|item| match item {
            FormItem::Field(field) if known_fields.iter().any(|known| field.key.word_eq(known)) => {
                None
            }
            FormItem::Field(field) => Some(field_extension("clrmamepro", "", field)),
            FormItem::Flag(flag) => Some(flag_extension("clrmamepro", "", flag)),
            FormItem::Form(child) => Some(form_extension("clrmamepro", "", child)),
        })
        .collect()
}

impl<'a> Lexer<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    fn tokenize(mut self) -> crate::Result<Vec<Token<'a>>> {
        let mut tokens = Vec::new();
        while let Some(character) = self.peek() {
            if character.is_whitespace() {
                self.bump();
                continue;
            }
            let start = self.offset;
            let location = self.location();
            let kind = match character {
                '(' => {
                    self.bump();
                    TokenKind::LeftParen
                }
                ')' => {
                    self.bump();
                    TokenKind::RightParen
                }
                ';' => {
                    while self.peek().is_some_and(|next| next != '\n') {
                        self.bump();
                    }
                    TokenKind::Comment
                }
                '"' => TokenKind::Quoted(self.quoted_value(location)?),
                _ => {
                    while self.peek().is_some_and(|next| {
                        !next.is_whitespace() && !matches!(next, '(' | ')' | ';')
                    }) {
                        self.bump();
                    }
                    TokenKind::Word
                }
            };
            let raw = self.input.get(start..self.offset).ok_or_else(|| {
                parse_error("invalid token boundary", "document", None, Some(location))
            })?;
            tokens.push(Token {
                kind,
                raw,
                start,
                end: self.offset,
                location,
            });
            if tokens.len() > MAX_TOKENS {
                return Err(parse_error(
                    format!("DAT exceeds the {MAX_TOKENS}-token limit"),
                    "document",
                    None,
                    Some(location),
                ));
            }
        }
        Ok(tokens)
    }

    fn quoted_value(&mut self, location: RecordLocation) -> crate::Result<String> {
        self.bump();
        let mut value = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(value),
                Some('\\') if self.peek() == Some('"') => {
                    self.bump();
                    value.push('"');
                }
                Some(character) => value.push(character),
                None => {
                    return Err(parse_error(
                        "unterminated quoted value",
                        "document",
                        None,
                        Some(location),
                    ));
                }
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.offset..)?.chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }

    const fn location(&self) -> RecordLocation {
        RecordLocation {
            line: self.line,
            column: self.column,
        }
    }
}

struct Parser<'a> {
    input: &'a str,
    tokens: Vec<Token<'a>>,
    comments: Vec<Token<'a>>,
    cursor: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, tokens: Vec<Token<'a>>) -> Self {
        let comments = tokens
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::Comment))
            .cloned()
            .collect();
        Self {
            input,
            tokens,
            comments,
            cursor: 0,
        }
    }

    fn parse_document(&mut self) -> crate::Result<Vec<Form<'a>>> {
        let mut forms = Vec::new();
        loop {
            self.skip_comments();
            if self.cursor == self.tokens.len() {
                return Ok(forms);
            }
            forms.push(self.parse_form(0)?);
        }
    }

    fn parse_form(&mut self, depth: usize) -> crate::Result<Form<'a>> {
        if depth >= MAX_FORM_DEPTH {
            return Err(parse_error(
                format!("DAT nesting exceeds {MAX_FORM_DEPTH} forms"),
                "document",
                None,
                self.peek_token().map(|token| token.location),
            ));
        }
        self.skip_comments();
        let tag = self.take_word("expected a form name")?;
        let start = tag.start;
        self.take_left_paren("expected '(' after form name")?;
        let mut items = Vec::new();
        loop {
            self.skip_comments();
            match self.peek_token() {
                Some(Token {
                    kind: TokenKind::RightParen,
                    ..
                }) => {
                    let close = self.take_token().ok_or_else(|| {
                        parse_error("missing ')'", "document", None, Some(tag.location))
                    })?;
                    let raw = self.input.get(start..close.end).ok_or_else(|| {
                        parse_error(
                            "invalid form token range",
                            "document",
                            None,
                            Some(tag.location),
                        )
                    })?;
                    return Ok(Form { tag, items, raw });
                }
                None => {
                    return Err(parse_error(
                        format!("unterminated {} form", tag.value()),
                        tag.value(),
                        None,
                        Some(tag.location),
                    ));
                }
                Some(_) => {}
            }
            let item_start = self.cursor;
            let key = self.take_word("expected a keyword or nested form")?;
            self.skip_comments();
            if matches!(
                self.peek_token().map(|token| &token.kind),
                Some(TokenKind::LeftParen)
            ) {
                self.cursor = item_start;
                items.push(FormItem::Form(self.parse_form(depth + 1)?));
            } else if key.word_eq("nodump") || key.word_eq("baddump") {
                items.push(FormItem::Flag(key));
            } else if let Some(next) = self.peek_token() {
                if matches!(next.kind, TokenKind::RightParen) {
                    if is_value_keyword(&tag, &key) {
                        let record_name = form_record_name(&items);
                        return Err(parse_error(
                            format!("keyword {} has no value", key.value()),
                            tag.value(),
                            record_name,
                            Some(key.location),
                        ));
                    }
                    items.push(FormItem::Flag(key));
                } else if !is_value_keyword(&tag, &key) && is_value_keyword(&tag, next) {
                    items.push(FormItem::Flag(key));
                } else if let Some(value) = self.take_value() {
                    items.push(FormItem::Field(Field { key, value }));
                } else {
                    return Err(parse_error(
                        format!("keyword {} has no value", key.value()),
                        tag.value(),
                        form_record_name(&items),
                        Some(key.location),
                    ));
                }
            } else {
                return Err(parse_error(
                    format!("keyword {} has no value", key.value()),
                    tag.value(),
                    form_record_name(&items),
                    Some(key.location),
                ));
            }
        }
    }

    fn take_word(&mut self, message: &str) -> crate::Result<Token<'a>> {
        self.skip_comments();
        match self.peek_token() {
            Some(Token {
                kind: TokenKind::Word,
                ..
            }) => self
                .take_token()
                .ok_or_else(|| parse_error(message, "document", None, None)),
            token => Err(parse_error(
                message,
                "document",
                None,
                token.map(|token| token.location),
            )),
        }
    }

    fn take_value(&mut self) -> Option<Token<'a>> {
        if matches!(
            self.peek_token().map(|token| &token.kind),
            Some(TokenKind::Word | TokenKind::Quoted(_))
        ) {
            self.take_token()
        } else {
            None
        }
    }

    fn take_left_paren(&mut self, message: &str) -> crate::Result<()> {
        self.skip_comments();
        if matches!(
            self.peek_token().map(|token| &token.kind),
            Some(TokenKind::LeftParen)
        ) {
            self.cursor += 1;
            Ok(())
        } else {
            Err(parse_error(
                message,
                "document",
                None,
                self.peek_token().map(|token| token.location),
            ))
        }
    }

    fn skip_comments(&mut self) {
        while matches!(
            self.peek_token().map(|token| &token.kind),
            Some(TokenKind::Comment)
        ) {
            self.cursor += 1;
        }
    }

    fn peek_token(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.cursor)
    }

    fn take_token(&mut self) -> Option<Token<'a>> {
        let token = self.tokens.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token)
    }
}

fn is_value_keyword(form: &Token<'_>, keyword: &Token<'_>) -> bool {
    if form.word_eq("rom") {
        [
            "name", "size", "crc", "crc32", "md5", "sha1", "merge", "status", "date", "serial",
        ]
        .iter()
        .any(|field| keyword.word_eq(field))
    } else if form.word_eq("game") || form.word_eq("set") {
        [
            "name",
            "cloneof",
            "description",
            "year",
            "region",
            "releaseyear",
            "releasemonth",
            "releaseday",
            "manufacturer",
            "rebuildto",
            "sampleof",
            "sample",
            "serial",
        ]
        .iter()
        .any(|field| keyword.word_eq(field))
    } else if form.word_eq("clrmamepro") {
        [
            "name",
            "description",
            "version",
            "date",
            "author",
            "email",
            "comment",
            "category",
            "homepage",
            "url",
            "header",
            "forcemerging",
            "forcezipping",
            "forcepacking",
            "forcenodump",
        ]
        .iter()
        .any(|field| keyword.word_eq(field))
    } else {
        false
    }
}

fn field_value(form: &Form<'_>, field_name: &str) -> crate::Result<Option<FieldValue>> {
    field_value_scoped(form, field_name, form.tag.value(), None)
}

fn field_value_scoped(
    form: &Form<'_>,
    field_name: &str,
    record_kind: &str,
    record_name: Option<&str>,
) -> crate::Result<Option<FieldValue>> {
    let field = single_field(form, field_name, record_kind, record_name)?;
    Ok(field.map(|field| FieldValue {
        source_name: field.key.value().to_owned(),
        value: field.value.value().to_owned(),
        quoted: matches!(field.value.kind, TokenKind::Quoted(_)),
        order: form
            .items
            .iter()
            .position(
                |item| matches!(item, FormItem::Field(candidate) if std::ptr::eq(candidate, field)),
            )
            .unwrap_or_default(),
        location: field.value.location,
    }))
}

fn parse_header(form: &Form<'_>, source_order: usize) -> crate::Result<Header> {
    Ok(Header {
        source_name: form.tag.value().to_owned(),
        source_order,
        name: field_value(form, "name")?,
        description: field_value(form, "description")?,
        version: field_value(form, "version")?,
        date: field_value(form, "date")?,
        author: field_value(form, "author")?,
        email: field_value(form, "email")?,
        homepage: field_value(form, "homepage")?,
        url: field_value(form, "url")?,
        comment: field_value(form, "comment")?,
        category: field_value(form, "category")?,
        directives: HeaderDirectives {
            header: field_value(form, "header")?,
            forcemerging: field_value(form, "forcemerging")?,
            forcezipping: field_value(form, "forcezipping")?,
            forcepacking: field_value(form, "forcepacking")?,
            forcenodump: field_value(form, "forcenodump")?,
        },
        location: form.tag.location,
    })
}

fn form_record_name<'items>(items: &'items [FormItem<'_>]) -> Option<&'items str> {
    items.iter().find_map(|item| match item {
        FormItem::Field(field) if field.key.word_eq("name") => Some(field.value.value()),
        _ => None,
    })
}

fn parse_set(form: &Form<'_>, document_order: usize) -> crate::Result<Set> {
    let mut names = form.fields("name");
    let name_field = names.next().ok_or_else(|| {
        parse_error(
            "set is missing name",
            form.tag.value(),
            None,
            Some(form.tag.location),
        )
    })?;
    let name = name_field.value.value().to_owned();
    if names.next().is_some() {
        return Err(parse_error(
            "duplicate name field",
            form.tag.value(),
            Some(&name),
            Some(form.tag.location),
        ));
    }
    let native = parse_set_facts(form, &name, document_order)?;
    let parent = native.cloneof.as_ref().map(|field| field.value.clone());
    let mut metadata: BTreeMap<String, Value> = BTreeMap::new();
    for (name, value) in [
        ("description", &native.description),
        ("year", &native.year),
        ("manufacturer", &native.manufacturer),
    ] {
        if let Some(value) = value {
            metadata.insert(name.into(), json!(value.value));
        }
    }
    metadata.insert("source_tokens".into(), json!(form.raw));

    let mut assets = Vec::new();
    let mut extensions = Vec::new();
    for (set_order, item) in form.items.iter().enumerate() {
        match item {
            FormItem::Form(child) if child.tag.word_eq("rom") => {
                assets.push(parse_asset(child, &name, set_order)?);
            }
            FormItem::Form(child) => extensions.push(form_extension("set", &name, child)),
            FormItem::Field(field)
                if [
                    "name",
                    "cloneof",
                    "description",
                    "year",
                    "region",
                    "releaseyear",
                    "releasemonth",
                    "releaseday",
                    "manufacturer",
                    "rebuildto",
                    "sampleof",
                    "sample",
                    "serial",
                ]
                .iter()
                .any(|known| field.key.word_eq(known)) => {}
            FormItem::Field(field) => extensions.push(field_extension("set", &name, field)),
            FormItem::Flag(flag) => extensions.push(flag_extension("set", &name, flag)),
        }
    }
    Ok(Set {
        name,
        parent,
        metadata: json!(metadata),
        location: form.tag.location,
        assets,
        extensions,
        native,
    })
}

fn parse_set_facts(form: &Form<'_>, name: &str, document_order: usize) -> crate::Result<SetFacts> {
    let field = |field_name: &'static str| field_value_scoped(form, field_name, "set", Some(name));
    Ok(SetFacts {
        source_block: form.tag.value().to_owned(),
        document_order,
        name: field("name")?,
        cloneof: field("cloneof")?,
        description: field("description")?,
        year: field("year")?,
        region: field("region")?,
        release_year_text: field("releaseyear")?,
        release_month_text: field("releasemonth")?,
        release_day_text: field("releaseday")?,
        manufacturer: field("manufacturer")?,
        rebuildto: field("rebuildto")?,
        sampleof: field("sampleof")?,
        serial: field("serial")?,
        samples: form
            .items
            .iter()
            .enumerate()
            .filter_map(|(order, item)| match item {
                FormItem::Field(field) if field.key.word_eq("sample") => Some(FieldValue {
                    source_name: field.key.value().to_owned(),
                    value: field.value.value().to_owned(),
                    quoted: matches!(field.value.kind, TokenKind::Quoted(_)),
                    order,
                    location: field.value.location,
                }),
                _ => None,
            })
            .collect(),
    })
}

fn parse_asset(form: &Form<'_>, set_name: &str, set_order: usize) -> crate::Result<Asset> {
    let name = single_field(form, "name", "rom", Some(set_name))?
        .ok_or_else(|| {
            parse_error(
                "rom is missing name",
                "rom",
                Some(set_name),
                Some(form.tag.location),
            )
        })?
        .value
        .value()
        .to_owned();
    let field = |field_name: &'static str| single_field(form, field_name, "rom", Some(&name));
    let size = field("size")?
        .map(|field| {
            let value = field.value.value();
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(parse_error(
                    format!("invalid ROM size {value:?}"),
                    "rom",
                    Some(&name),
                    Some(field.value.location),
                ));
            }
            let size = value.parse::<i64>().map_err(|_| {
                parse_error(
                    format!("invalid ROM size {value:?}"),
                    "rom",
                    Some(&name),
                    Some(field.value.location),
                )
            })?;
            u64::try_from(size).map_err(|_| {
                parse_error(
                    format!("invalid ROM size {value:?}"),
                    "rom",
                    Some(&name),
                    Some(field.value.location),
                )
            })
        })
        .transpose()?;
    let crc = digest_field(form, "crc", 8, "rom", &name)?;
    let crc32 = digest_field(form, "crc32", 8, "rom", &name)?;
    let crc = match (crc, crc32) {
        (Some(crc), Some(crc32)) if crc == crc32 => Some(crc),
        (Some(_), Some(_)) | (None, None) => None,
        (Some(crc), None) | (None, Some(crc)) => Some(crc),
    };
    let md5 = digest_field(form, "md5", 32, "rom", &name)?;
    let sha1 = digest_field(form, "sha1", 40, "rom", &name)?;
    let merge = single_field(form, "merge", "rom", Some(&name))?
        .map(|field| field.value.value().to_owned());
    let status = parse_asset_status(form, &name)?;
    let native = parse_asset_facts(form, set_order)?;
    let known_fields = [
        "name", "size", "crc", "crc32", "md5", "sha1", "merge", "status", "date", "serial",
    ];
    let mut extensions = Vec::new();
    for item in &form.items {
        match item {
            FormItem::Field(field) if known_fields.iter().any(|known| field.key.word_eq(known)) => {
            }
            FormItem::Flag(flag) if flag.word_eq("nodump") || flag.word_eq("baddump") => {}
            FormItem::Field(field) => extensions.push(field_extension("rom", &name, field)),
            FormItem::Flag(flag) => extensions.push(flag_extension("rom", &name, flag)),
            FormItem::Form(child) => extensions.push(form_extension("rom", &name, child)),
        }
    }
    let mut metadata: BTreeMap<String, Value> = BTreeMap::new();
    metadata.insert("source_tokens".into(), json!(form.raw));
    Ok(Asset {
        name,
        size,
        crc,
        md5,
        sha1,
        merge,
        status,
        location: form.tag.location,
        metadata: json!(metadata),
        extensions,
        native,
    })
}

fn parse_asset_status(form: &Form<'_>, name: &str) -> crate::Result<Option<String>> {
    let explicit_status = single_field(form, "status", "rom", Some(name))?
        .map(|field| field.value.value().to_owned());
    let nodump = form.flags("nodump");
    let baddump = form.flags("baddump");
    if nodump.len() > 1 || baddump.len() > 1 {
        return Err(parse_error(
            "duplicate nodump or baddump flag",
            "rom",
            Some(name),
            Some(form.tag.location),
        ));
    }
    if !nodump.is_empty() && !baddump.is_empty()
        || explicit_status.is_some() && (!nodump.is_empty() || !baddump.is_empty())
    {
        return Ok(None);
    }
    Ok(explicit_status.or_else(|| {
        if !nodump.is_empty() {
            Some("nodump".into())
        } else if !baddump.is_empty() {
            Some("baddump".into())
        } else {
            None
        }
    }))
}

fn parse_asset_facts(form: &Form<'_>, set_order: usize) -> crate::Result<AssetFacts> {
    Ok(AssetFacts {
        set_order,
        name: field_value(form, "name")?,
        size: field_value(form, "size")?,
        crc: field_value(form, "crc")?,
        crc32: field_value(form, "crc32")?,
        md5: field_value(form, "md5")?,
        sha1: field_value(form, "sha1")?,
        merge: field_value(form, "merge")?,
        date: field_value(form, "date")?,
        serial: field_value(form, "serial")?,
        status_field: field_value(form, "status")?,
        nodump: flag_value(form, "nodump")?,
        baddump: flag_value(form, "baddump")?,
    })
}

fn flag_value(form: &Form<'_>, name: &str) -> crate::Result<Option<FlagValue>> {
    let mut flags = form
        .items
        .iter()
        .enumerate()
        .filter_map(|(order, item)| match item {
            FormItem::Flag(flag) if flag.word_eq(name) => Some((order, flag)),
            _ => None,
        });
    let Some((order, flag)) = flags.next() else {
        return Ok(None);
    };
    if flags.next().is_some() {
        return Err(parse_error(
            format!("duplicate {name} flag"),
            "rom",
            None,
            Some(form.tag.location),
        ));
    }
    Ok(Some(FlagValue {
        source_name: flag.value().to_owned(),
        order,
        location: flag.location,
    }))
}

impl<'src> Form<'src> {
    fn fields<'form>(
        &'form self,
        name: &'form str,
    ) -> impl Iterator<Item = &'form Field<'src>> + 'form {
        self.items.iter().filter_map(move |item| match item {
            FormItem::Field(field) if field.key.word_eq(name) => Some(field),
            _ => None,
        })
    }

    fn flags(&self, name: &str) -> Vec<&Token<'src>> {
        self.items
            .iter()
            .filter_map(|item| match item {
                FormItem::Flag(flag) if flag.word_eq(name) => Some(flag),
                _ => None,
            })
            .collect()
    }
}

impl Token<'_> {
    fn value(&self) -> &str {
        match &self.kind {
            TokenKind::Word | TokenKind::Comment => self.raw,
            TokenKind::Quoted(value) => value,
            TokenKind::LeftParen => "(",
            TokenKind::RightParen => ")",
        }
    }

    const fn word_eq(&self, expected: &str) -> bool {
        matches!(&self.kind, TokenKind::Word) && self.raw.eq_ignore_ascii_case(expected)
    }
}

fn single_field<'form, 'src>(
    form: &'form Form<'src>,
    field_name: &'form str,
    record_kind: &str,
    record_name: Option<&str>,
) -> crate::Result<Option<&'form Field<'src>>> {
    let mut fields = form.fields(field_name);
    let field = fields.next();
    if fields.next().is_some() {
        return Err(parse_error(
            format!("duplicate {field_name} field"),
            record_kind,
            record_name,
            Some(form.tag.location),
        ));
    }
    Ok(field)
}

fn digest_field(
    form: &Form<'_>,
    field_name: &str,
    expected_hex_length: usize,
    record_kind: &str,
    record_name: &str,
) -> crate::Result<Option<Vec<u8>>> {
    let Some(field) = single_field(form, field_name, record_kind, Some(record_name))? else {
        return Ok(None);
    };
    let value = field.value.value();
    if value.len() != expected_hex_length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(parse_error(
            format!("invalid {field_name} digest {value:?}"),
            record_kind,
            Some(record_name),
            Some(field.value.location),
        ));
    }
    hex::decode(value).map(Some).map_err(|error| {
        parse_error(
            format!("invalid {field_name} digest: {error}"),
            record_kind,
            Some(record_name),
            Some(field.value.location),
        )
    })
}

fn field_extension(record_kind: &str, record_name: &str, field: &Field<'_>) -> Extension {
    Extension {
        record_kind: record_kind.into(),
        record_name: Some(record_name.into()),
        field_name: field.key.value().into(),
        value: json!({"key_token": field.key.raw, "value_token": field.value.raw}),
        location: field.key.location,
    }
}

fn flag_extension(record_kind: &str, record_name: &str, flag: &Token<'_>) -> Extension {
    Extension {
        record_kind: record_kind.into(),
        record_name: Some(record_name.into()),
        field_name: flag.value().into(),
        value: json!({"raw_token": flag.raw}),
        location: flag.location,
    }
}

fn form_extension(record_kind: &str, record_name: &str, form: &Form<'_>) -> Extension {
    Extension {
        record_kind: record_kind.into(),
        record_name: Some(record_name.into()),
        field_name: format!("form:{}", form.tag.value()),
        value: json!({"raw_tokens": form.raw}),
        location: form.tag.location,
    }
}

fn parse_error(
    message: impl Into<String>,
    record_kind: &str,
    record_name: Option<&str>,
    location: Option<RecordLocation>,
) -> crate::Error {
    crate::Error::CatalogParse {
        message: message.into(),
        record_kind: Some(record_kind.to_owned()),
        record_name: record_name.map(str::to_owned),
        line: location.map(|location| location.line),
        column: location.map(|location| location.column),
    }
}

#[cfg(test)]
fn parse_error_unscoped(message: impl Into<String>) -> crate::Error {
    crate::Error::CatalogParse {
        message: message.into(),
        record_kind: Some("document".into()),
        record_name: None,
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_case_insensitive_tags_partial_hashes_and_dump_flags() -> crate::Result<()> {
        let catalog = Catalog::parse(
            br#"clrmamepro ( name "Fixture" version 1 )
               GAME ( NAME "demo" ROM ( name "nodump.bin" nodump )
                 ROM ( name "partial.bin" SHA1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ) )"#,
        )?;
        assert_eq!(catalog.version.as_deref(), Some("1"));
        let set = catalog
            .sets
            .first()
            .ok_or_else(|| parse_error_unscoped("missing set"))?;
        assert_eq!(set.name, "demo");
        assert_eq!(set.assets.len(), 2);
        assert_eq!(set.assets[0].status.as_deref(), Some("nodump"));
        assert_eq!(set.assets[1].size, None);
        assert_eq!(set.assets[1].sha1.as_ref().map(Vec::len), Some(20));
        Ok(())
    }

    #[test]
    fn retains_comments_unknown_tokens_and_original_form_tokens() -> crate::Result<()> {
        let catalog = Catalog::parse(
            b"; top\nclrmamepro ( version v1 )\ngame ( name set rom ( name x.bin futuretag \"opaque\" mysteryflag crc 12345678 ) )",
        )?;
        assert_eq!(catalog.comments.len(), 1);
        assert_eq!(catalog.comments[0].text, "; top");
        assert_eq!(catalog.comments[0].location.line, 1);
        assert!(
            catalog
                .extensions
                .iter()
                .all(|extension| extension.field_name != "comment")
        );
        let set = catalog
            .sets
            .first()
            .ok_or_else(|| parse_error_unscoped("missing set"))?;
        assert!(
            set.metadata["source_tokens"]
                .as_str()
                .is_some_and(|raw| raw.contains("name set"))
        );
        let ext = set
            .assets
            .first()
            .and_then(|asset| asset.extensions.first())
            .ok_or_else(|| parse_error_unscoped("missing extension"))?;
        assert_eq!(ext.field_name, "futuretag");
        assert_eq!(ext.value["value_token"], "\"opaque\"");
        assert!(ext.location.line > 0);
        assert!(
            set.assets[0]
                .extensions
                .iter()
                .any(|extension| extension.field_name == "mysteryflag"
                    && extension.value["raw_token"] == "mysteryflag")
        );
        assert_eq!(set.assets[0].crc, Some(vec![0x12, 0x34, 0x56, 0x78]));
        Ok(())
    }

    #[test]
    fn retains_set_provenance_fields_and_form_ordinals() -> crate::Result<()> {
        let catalog = Catalog::parse(
            b"; first\nunknown ()\nClRmAmEpRo ( version 1 comment \"header text\" )\nGaMe ( NAME \"MixedCase\" CLONEOF \"Parent\" REGION \"\" RELEASEYEAR 0000 RELEASEMONTH 09 RELEASEDAY 00 SERIAL \"AbC-01\" rom ( name first.bin ) note value rom ( name second.bin ) ) ; last",
        )?;
        let header = catalog
            .header
            .as_ref()
            .ok_or_else(|| parse_error_unscoped("missing header"))?;
        assert_eq!(header.source_name, "ClRmAmEpRo");
        assert_eq!(header.source_order, 1);
        assert_eq!(
            header.comment.as_ref().map(|field| field.value.as_str()),
            Some("header text")
        );
        let set = catalog
            .sets
            .first()
            .ok_or_else(|| parse_error_unscoped("missing set"))?;
        assert_eq!(set.native.source_block, "GaMe");
        assert_eq!(set.native.document_order, 2);
        let name = set
            .native
            .name
            .as_ref()
            .ok_or_else(|| parse_error_unscoped("missing native set name"))?;
        assert_eq!(
            (name.source_name.as_str(), name.value.as_str()),
            ("NAME", "MixedCase")
        );
        let cloneof = set
            .native
            .cloneof
            .as_ref()
            .ok_or_else(|| parse_error_unscoped("missing cloneof"))?;
        assert_eq!(cloneof.value, "Parent");
        for (field, source_name, value) in [
            (set.native.region.as_ref(), "REGION", ""),
            (set.native.release_year_text.as_ref(), "RELEASEYEAR", "0000"),
            (set.native.release_month_text.as_ref(), "RELEASEMONTH", "09"),
            (set.native.release_day_text.as_ref(), "RELEASEDAY", "00"),
            (set.native.serial.as_ref(), "SERIAL", "AbC-01"),
        ] {
            let field = field.ok_or_else(|| parse_error_unscoped("missing typed set field"))?;
            assert_eq!(field.source_name, source_name);
            assert_eq!(field.value, value);
            assert!(field.location.line > 0);
        }
        assert_eq!(set.assets.len(), 2);
        assert_eq!(set.assets[0].native.set_order, 7);
        assert_eq!(set.assets[1].native.set_order, 9);
        assert_eq!(catalog.comments.len(), 2);
        assert_eq!(catalog.comments[0].text, "; first");
        assert_eq!(catalog.comments[1].text, "; last");
        assert!(catalog.comments[0].location.line < catalog.comments[1].location.line);
        Ok(())
    }

    #[test]
    fn set_typed_fields_reject_duplicates_and_missing_values_with_set_context() {
        for field in [
            "region",
            "releaseyear",
            "releasemonth",
            "releaseday",
            "serial",
        ] {
            let duplicate = format!("game ( name chosen {field} one {field} two )");
            assert!(
                matches!(
                    Catalog::parse(duplicate.as_bytes()),
                    Err(crate::Error::CatalogParse {
                        record_kind: Some(kind),
                        record_name: Some(name),
                        ..
                    }) if kind == "set" && name == "chosen"
                ),
                "duplicate {field} should include set context"
            );

            let missing = format!("game ( name chosen {field} )");
            assert!(
                matches!(
                    Catalog::parse(missing.as_bytes()),
                    Err(crate::Error::CatalogParse {
                        record_kind: Some(kind),
                        record_name: Some(name),
                        ..
                    }) if kind == "game" && name == "chosen"
                ),
                "missing {field} value should include set context"
            );
        }
    }

    #[test]
    fn accepts_matching_crc_and_crc32_declarations() -> crate::Result<()> {
        let catalog =
            Catalog::parse(b"game ( name set rom ( name x crc 12345678 crc32 12345678 ) )")?;
        let asset = catalog
            .sets
            .first()
            .and_then(|set| set.assets.first())
            .ok_or_else(|| parse_error_unscoped("missing asset"))?;
        assert_eq!(asset.crc, Some(vec![0x12, 0x34, 0x56, 0x78]));
        assert!(!asset.native.has_conflicting_declarations());
        Ok(())
    }

    #[test]
    fn retains_native_asset_field_spelling_values_and_order() -> crate::Result<()> {
        let catalog = Catalog::parse(
            b"game ( name set rom ( NAME \"X.bin\" SIZE \"0001\" CRC \"AABBCCDD\" CRC32 aabbccdd MD5 0123456789abcdef0123456789abcdef SHA1 0123456789abcdef0123456789abcdef01234567 MERGE \"Parent.BIN\" DATE \"Sep 9\" SERIAL \"MiXeD\" STATUS \"good\" NoDuMp ) )",
        )?;
        let asset = catalog
            .sets
            .first()
            .and_then(|set| set.assets.first())
            .ok_or_else(|| parse_error_unscoped("missing asset"))?;
        let facts = &asset.native;
        let fields = [
            facts.name.as_ref(),
            facts.size.as_ref(),
            facts.crc.as_ref(),
            facts.crc32.as_ref(),
            facts.md5.as_ref(),
            facts.sha1.as_ref(),
            facts.merge.as_ref(),
            facts.date.as_ref(),
            facts.serial.as_ref(),
            facts.status_field.as_ref(),
        ];
        let expected = [
            ("NAME", "X.bin", true),
            ("SIZE", "0001", true),
            ("CRC", "AABBCCDD", true),
            ("CRC32", "aabbccdd", false),
            ("MD5", "0123456789abcdef0123456789abcdef", false),
            ("SHA1", "0123456789abcdef0123456789abcdef01234567", false),
            ("MERGE", "Parent.BIN", true),
            ("DATE", "Sep 9", true),
            ("SERIAL", "MiXeD", true),
            ("STATUS", "good", true),
        ];
        for (order, (field, (source_name, value, quoted))) in
            fields.into_iter().zip(expected).enumerate()
        {
            let field = field.ok_or_else(|| parse_error_unscoped("missing native field"))?;
            assert_eq!(field.source_name, source_name);
            assert_eq!(field.value, value);
            assert_eq!(field.quoted, quoted);
            assert_eq!(field.order, order);
            assert!(field.location.line > 0);
        }
        let nodump = facts
            .nodump
            .as_ref()
            .ok_or_else(|| parse_error_unscoped("missing nodump flag"))?;
        assert_eq!(nodump.source_name, "NoDuMp");
        assert_eq!(nodump.order, 10);
        assert!(nodump.location.line > 0);
        assert_eq!(facts.baddump, None);
        assert!(facts.has_conflicting_declarations());
        Ok(())
    }

    #[test]
    fn differing_crc_aliases_are_retained_without_an_effective_crc() -> crate::Result<()> {
        let catalog =
            Catalog::parse(b"game ( name set rom ( name x crc 12345678 crc32 87654321 ) )")?;
        let asset = catalog
            .sets
            .first()
            .and_then(|set| set.assets.first())
            .ok_or_else(|| parse_error_unscoped("missing asset"))?;
        assert_eq!(asset.crc, None);
        assert_eq!(
            asset.native.crc.as_ref().map(|field| field.value.as_str()),
            Some("12345678")
        );
        assert_eq!(
            asset
                .native
                .crc32
                .as_ref()
                .map(|field| field.value.as_str()),
            Some("87654321")
        );
        assert!(asset.native.has_conflicting_declarations());
        Ok(())
    }

    #[test]
    fn retains_conflicting_dump_markers_and_leaves_effective_status_unset() -> crate::Result<()> {
        let catalog = Catalog::parse(
            b"game ( name set rom ( name x NoDuMp bAdDuMp ) rom ( name y status \"good\" nodump ) )",
        )?;
        let assets = &catalog
            .sets
            .first()
            .ok_or_else(|| parse_error_unscoped("missing set"))?
            .assets;
        assert_eq!(assets[0].status, None);
        assert_eq!(
            assets[0]
                .native
                .nodump
                .as_ref()
                .map(|flag| flag.source_name.as_str()),
            Some("NoDuMp")
        );
        assert_eq!(
            assets[0]
                .native
                .baddump
                .as_ref()
                .map(|flag| flag.source_name.as_str()),
            Some("bAdDuMp")
        );
        assert!(assets[0].native.has_conflicting_declarations());
        assert_eq!(assets[1].status, None);
        assert!(assets[1].native.has_conflicting_declarations());
        assert!(Catalog::parse(b"game ( name set rom ( name duplicate nodump NODUMP ) )").is_err());
        Ok(())
    }

    #[test]
    fn rejects_non_ascii_decimal_and_out_of_range_sizes() {
        for size in ["+1", "\" 1\"", "١", "9223372036854775808"] {
            let input = format!("game ( name set rom ( name x size {size} ) )");
            assert!(
                matches!(
                    Catalog::parse(input.as_bytes()),
                    Err(crate::Error::CatalogParse {
                        record_kind: Some(kind),
                        record_name: Some(name),
                        line: Some(_),
                        column: Some(_),
                        ..
                    }) if kind == "rom" && name == "x"
                ),
                "size {size:?} should be rejected with ROM context"
            );
        }
    }

    #[test]
    fn accepts_leading_zero_decimal_sizes() -> crate::Result<()> {
        let catalog = Catalog::parse(b"game ( name set rom ( name x size 0001 ) )")?;
        let asset = catalog
            .sets
            .first()
            .and_then(|set| set.assets.first())
            .ok_or_else(|| parse_error_unscoped("missing asset"))?;
        assert_eq!(asset.size, Some(1));
        assert_eq!(
            asset.native.size.as_ref().map(|field| field.value.as_str()),
            Some("0001")
        );
        Ok(())
    }

    #[test]
    fn malformed_and_ambiguous_records_have_record_context() {
        let duplicate = Catalog::parse(b"game ( name one name two )");
        assert!(
            matches!(duplicate, Err(crate::Error::CatalogParse { record_name: Some(name), .. }) if name == "one")
        );
        let unclosed = Catalog::parse(b"game ( name one");
        assert!(
            matches!(unclosed, Err(crate::Error::CatalogParse { record_kind: Some(kind), .. }) if kind == "game")
        );
        let missing_size = Catalog::parse(b"game ( name one rom ( name x size ) )");
        assert!(matches!(
            missing_size,
            Err(crate::Error::CatalogParse { record_kind: Some(kind), record_name: Some(name), .. })
                if kind == "rom" && name == "x"
        ));
    }
}
