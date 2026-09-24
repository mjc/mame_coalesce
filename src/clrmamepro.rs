use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::logiqx::RecordLocation;

const MAX_TOKENS: usize = 1_000_000;
const MAX_FORM_DEPTH: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub version: Option<String>,
    pub sets: Vec<Set>,
    pub extensions: Vec<Extension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Set {
    pub name: String,
    pub parent: Option<String>,
    pub metadata: Value,
    pub location: RecordLocation,
    pub assets: Vec<Asset>,
    pub extensions: Vec<Extension>,
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
enum TokenKind {
    Word(String),
    Quoted(String),
    LeftParen,
    RightParen,
    Comment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    raw: String,
    start: usize,
    end: usize,
    location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Field {
    key: Token,
    value: Token,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FormItem {
    Field(Field),
    Flag(Token),
    Form(Form),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Form {
    tag: Token,
    items: Vec<FormItem>,
    raw: String,
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
        let mut headers = forms.iter().filter(|form| form.tag.word_eq("clrmamepro"));
        let header = headers.next();
        if headers.next().is_some() {
            return Err(parse_error(
                "multiple clrmamepro headers are ambiguous",
                "document",
                None,
                header.map(|form| form.tag.location),
            ));
        }
        let version = header
            .map(|form| single_field(form, "version", "clrmamepro", None))
            .transpose()?
            .flatten()
            .map(|field| field.value.value().to_owned());

        let mut sets = Vec::new();
        let mut extensions = parser
            .comments
            .into_iter()
            .map(|token| Extension {
                record_kind: "comment".into(),
                record_name: None,
                field_name: "comment".into(),
                value: json!({"raw_token": token.raw}),
                location: token.location,
            })
            .collect::<Vec<_>>();
        if let Some(header) = header {
            extensions.push(Extension {
                record_kind: "clrmamepro".into(),
                record_name: None,
                field_name: "source_tokens".into(),
                value: json!({"raw_form": header.raw}),
                location: header.tag.location,
            });
        }
        for form in &forms {
            if form.tag.word_eq("game") || form.tag.word_eq("set") {
                sets.push(parse_set(form)?);
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
            sets,
            extensions,
        })
    }
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

    fn tokenize(mut self) -> crate::Result<Vec<Token>> {
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
                    let mut word = String::new();
                    while self.peek().is_some_and(|next| {
                        !next.is_whitespace() && !matches!(next, '(' | ')' | ';')
                    }) {
                        if let Some(next) = self.bump() {
                            word.push(next);
                        }
                    }
                    TokenKind::Word(word)
                }
            };
            let raw = self
                .input
                .get(start..self.offset)
                .ok_or_else(|| {
                    parse_error("invalid token boundary", "document", None, Some(location))
                })?
                .to_owned();
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
    tokens: Vec<Token>,
    comments: Vec<Token>,
    cursor: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, tokens: Vec<Token>) -> Self {
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

    fn parse_document(&mut self) -> crate::Result<Vec<Form>> {
        let mut forms = Vec::new();
        loop {
            self.skip_comments();
            if self.cursor == self.tokens.len() {
                return Ok(forms);
            }
            forms.push(self.parse_form(0)?);
        }
    }

    fn parse_form(&mut self, depth: usize) -> crate::Result<Form> {
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
                    let raw = self
                        .input
                        .get(start..close.end)
                        .ok_or_else(|| {
                            parse_error(
                                "invalid form token range",
                                "document",
                                None,
                                Some(tag.location),
                            )
                        })?
                        .to_owned();
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
            } else {
                let value = self.take_value().ok_or_else(|| {
                    parse_error(
                        format!("keyword {} has no value", key.value()),
                        tag.value(),
                        None,
                        Some(key.location),
                    )
                })?;
                items.push(FormItem::Field(Field { key, value }));
            }
        }
    }

    fn take_word(&mut self, message: &str) -> crate::Result<Token> {
        self.skip_comments();
        match self.peek_token() {
            Some(Token {
                kind: TokenKind::Word(_),
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

    fn take_value(&mut self) -> Option<Token> {
        if matches!(
            self.peek_token().map(|token| &token.kind),
            Some(TokenKind::Word(_) | TokenKind::Quoted(_))
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

    fn peek_token(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn take_token(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token)
    }
}

fn parse_set(form: &Form) -> crate::Result<Set> {
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
    let parent = single_field(form, "cloneof", "set", Some(&name))?
        .map(|field| field.value.value().to_owned());
    let mut metadata: BTreeMap<String, Value> = BTreeMap::new();
    for field_name in ["description", "year", "manufacturer"] {
        if let Some(field) = single_field(form, field_name, "set", Some(&name))? {
            metadata.insert(field_name.into(), json!(field.value.value()));
        }
    }
    metadata.insert("source_tokens".into(), json!(form.raw));

    let mut assets = Vec::new();
    let mut extensions = Vec::new();
    for item in &form.items {
        match item {
            FormItem::Form(child) if child.tag.word_eq("rom") => {
                assets.push(parse_asset(child, &name)?);
            }
            FormItem::Form(child) => extensions.push(form_extension("set", &name, child)),
            FormItem::Field(field)
                if ["name", "cloneof", "description", "year", "manufacturer"]
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
    })
}

fn parse_asset(form: &Form, set_name: &str) -> crate::Result<Asset> {
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
            field.value.value().parse::<u64>().map_err(|_| {
                parse_error(
                    format!("invalid ROM size {:?}", field.value.value()),
                    "rom",
                    Some(&name),
                    Some(field.value.location),
                )
            })
        })
        .transpose()?;
    let crc = digest_field(form, "crc", 8, "rom", &name)?;
    let crc32 = digest_field(form, "crc32", 8, "rom", &name)?;
    if crc.is_some() && crc32.is_some() {
        return Err(parse_error(
            "both crc and crc32 are declared",
            "rom",
            Some(&name),
            Some(form.tag.location),
        ));
    }
    let md5 = digest_field(form, "md5", 32, "rom", &name)?;
    let sha1 = digest_field(form, "sha1", 40, "rom", &name)?;
    let merge = single_field(form, "merge", "rom", Some(&name))?
        .map(|field| field.value.value().to_owned());
    let explicit_status = single_field(form, "status", "rom", Some(&name))?
        .map(|field| field.value.value().to_owned());
    let nodump = form.flags("nodump");
    let baddump = form.flags("baddump");
    if nodump.len() > 1 || baddump.len() > 1 || (!nodump.is_empty() && !baddump.is_empty()) {
        return Err(parse_error(
            "duplicate or conflicting nodump/baddump flags",
            "rom",
            Some(&name),
            Some(form.tag.location),
        ));
    }
    if explicit_status.is_some() && (!nodump.is_empty() || !baddump.is_empty()) {
        return Err(parse_error(
            "status field conflicts with nodump/baddump flag",
            "rom",
            Some(&name),
            Some(form.tag.location),
        ));
    }
    let status = explicit_status.or_else(|| {
        if !nodump.is_empty() {
            Some("nodump".into())
        } else if !baddump.is_empty() {
            Some("baddump".into())
        } else {
            None
        }
    });
    let known_fields = [
        "name", "size", "crc", "crc32", "md5", "sha1", "merge", "status",
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
        crc: crc.or(crc32),
        md5,
        sha1,
        merge,
        status,
        location: form.tag.location,
        metadata: json!(metadata),
        extensions,
    })
}

impl Form {
    fn fields<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Field> + 'a {
        self.items.iter().filter_map(move |item| match item {
            FormItem::Field(field) if field.key.word_eq(name) => Some(field),
            _ => None,
        })
    }

    fn flags(&self, name: &str) -> Vec<&Token> {
        self.items
            .iter()
            .filter_map(|item| match item {
                FormItem::Flag(flag) if flag.word_eq(name) => Some(flag),
                _ => None,
            })
            .collect()
    }
}

impl Token {
    fn value(&self) -> &str {
        match &self.kind {
            TokenKind::Word(value) | TokenKind::Quoted(value) => value,
            TokenKind::LeftParen => "(",
            TokenKind::RightParen => ")",
            TokenKind::Comment => &self.raw,
        }
    }

    fn word_eq(&self, expected: &str) -> bool {
        matches!(&self.kind, TokenKind::Word(value) if value.eq_ignore_ascii_case(expected))
    }
}

fn single_field<'a>(
    form: &'a Form,
    field_name: &'a str,
    record_kind: &str,
    record_name: Option<&str>,
) -> crate::Result<Option<&'a Field>> {
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
    form: &Form,
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

fn field_extension(record_kind: &str, record_name: &str, field: &Field) -> Extension {
    Extension {
        record_kind: record_kind.into(),
        record_name: Some(record_name.into()),
        field_name: field.key.value().into(),
        value: json!({"key_token": field.key.raw, "value_token": field.value.raw}),
        location: field.key.location,
    }
}

fn flag_extension(record_kind: &str, record_name: &str, flag: &Token) -> Extension {
    Extension {
        record_kind: record_kind.into(),
        record_name: Some(record_name.into()),
        field_name: flag.value().into(),
        value: json!({"raw_token": flag.raw}),
        location: flag.location,
    }
}

fn form_extension(record_kind: &str, record_name: &str, form: &Form) -> Extension {
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
            b"; top\nclrmamepro ( version v1 )\ngame ( name set rom ( name x.bin futuretag \"opaque\" ) )",
        )?;
        assert_eq!(
            catalog
                .extensions
                .first()
                .map(|extension| extension.field_name.as_str()),
            Some("comment")
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
    }
}
