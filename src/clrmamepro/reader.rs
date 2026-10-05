use super::{
    Comment, Extension, Field, Form, FormItem, Header, Set, Token, TokenKind, form_record_name,
    header_extensions, is_value_keyword, parse_error, parse_header, parse_set,
};
use crate::logiqx::RecordLocation;

const MAX_TOKENS: usize = 1_000_000;
const MAX_FORM_DEPTH: usize = 128;

#[must_use = "the seal confirms the complete document reached validated EOF"]
/// Opaque confirmation that parsing reached a valid end of the document.
#[derive(Debug)]
pub struct EofSeal {
    header_present: bool,
    comment_count: usize,
    set_count: usize,
    _private: (),
}

impl EofSeal {
    /// Whether the document contained a `clrmamepro` header.
    #[must_use]
    pub const fn header_present(&self) -> bool {
        self.header_present
    }

    /// Number of comment tokens observed in the document.
    #[must_use]
    pub const fn comment_count(&self) -> usize {
        self.comment_count
    }

    /// Number of complete `game` or `set` forms observed in the document.
    #[must_use]
    pub const fn set_count(&self) -> usize {
        self.set_count
    }
}

/// One parser event, emitted in source order as each complete item is read.
pub enum Event {
    /// A lexical comment token.
    Comment(Comment),
    /// A validated header and its extensions.
    Header(Header, Vec<Extension>),
    /// A validated complete set/game form.
    Set(Set),
    /// An unknown top-level form retained as a document extension.
    Extension(Extension),
}

/// Receives validated parser events while the input is consumed.
pub trait EventConsumer {
    /// A terminal consumer or storage error. Parser errors are converted through `From`.
    type Error: From<crate::Error>;

    /// Consume one event. An error stops parsing immediately.
    fn consume(&mut self, event: Event) -> Result<(), Self::Error>;
}

/// Parse a `ClrMamePro` document incrementally, delivering validated events to `consumer`.
///
/// The returned seal is only available after the complete document and EOF have been
/// validated. A consumer error is terminal and is returned without parsing later input.
pub fn read_with<C>(bytes: &[u8], consumer: &mut C) -> Result<EofSeal, C::Error>
where
    C: EventConsumer,
{
    let input = std::str::from_utf8(bytes).map_err(|error| {
        C::Error::from(parse_error(
            format!("DAT is not valid UTF-8: {error}"),
            "document",
            None,
            None,
        ))
    })?;
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut parser = Parser::new(input, consumer);
    let mut form_order = 0_usize;
    let mut header_location = None;
    let mut header_count = 0_usize;
    let mut duplicate_header = None;
    let mut header_error = None;
    let mut set_error = None;
    let mut set_count = 0_usize;

    loop {
        let form = match parser.next_form() {
            Ok(Some(form)) => form,
            Ok(None) => break,
            Err(ParserFailure::Lexical(error)) => return Err(C::Error::from(error)),
            Err(ParserFailure::Form(error)) => {
                parser.disable_callbacks();
                match parser.drain_lexer() {
                    Ok(()) => return Err(C::Error::from(error)),
                    Err(lexical_error) => return Err(C::Error::from(lexical_error)),
                }
            }
            Err(ParserFailure::Consumer(error)) => return Err(error),
        };

        if form.tag.word_eq("clrmamepro") {
            if header_count == 0 {
                header_location = Some(form.tag.location);
                match parse_header(&form, form_order) {
                    Ok(header) => {
                        parser.emit(Event::Header(header, header_extensions(&form)))?;
                    }
                    Err(error) => {
                        header_error = Some(error);
                        parser.disable_callbacks();
                    }
                }
            } else if duplicate_header.is_none() {
                duplicate_header = Some(parse_error(
                    "multiple clrmamepro headers are ambiguous",
                    "document",
                    None,
                    header_location,
                ));
                parser.disable_callbacks();
            }
            header_count += 1;
        } else if form.tag.word_eq("game") || form.tag.word_eq("set") {
            set_count += 1;
            match parse_set(&form, form_order) {
                Ok(set) => parser.emit(Event::Set(set))?,
                Err(error) => {
                    if set_error.is_none() {
                        set_error = Some(error);
                    }
                    parser.disable_callbacks();
                }
            }
        } else {
            parser.emit(Event::Extension(Extension {
                record_kind: "document".into(),
                record_name: None,
                field_name: format!("form:{}", form.tag.value()),
                value: serde_json::json!({"raw_tokens": form.raw}),
                location: form.tag.location,
            }))?;
        }

        form_order = form_order.checked_add(1).ok_or_else(|| {
            C::Error::from(parse_error(
                "too many document forms",
                "document",
                None,
                Some(form.tag.location),
            ))
        })?;
    }

    validated_eof::<C>(
        header_count,
        parser.comment_count,
        set_count,
        duplicate_header,
        header_error,
        set_error,
    )
}

fn validated_eof<C: EventConsumer>(
    header_count: usize,
    comment_count: usize,
    set_count: usize,
    duplicate_header: Option<crate::Error>,
    header_error: Option<crate::Error>,
    set_error: Option<crate::Error>,
) -> Result<EofSeal, C::Error> {
    if let Some(error) = duplicate_header {
        return Err(C::Error::from(error));
    }
    if let Some(error) = header_error {
        return Err(C::Error::from(error));
    }
    if let Some(error) = set_error {
        return Err(C::Error::from(error));
    }
    if set_count == 0 {
        return Err(C::Error::from(parse_error(
            "DAT contains no set/game records",
            "document",
            None,
            None,
        )));
    }

    Ok(EofSeal {
        header_present: header_count != 0,
        comment_count,
        set_count,
        _private: (),
    })
}

enum ParserFailure<E> {
    Lexical(crate::Error),
    Form(crate::Error),
    Consumer(E),
}

struct Lexer<'a> {
    input: &'a str,
    offset: usize,
    line: i64,
    column: i64,
    token_count: usize,
}

impl<'a> Lexer<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
            line: 1,
            column: 1,
            token_count: 0,
        }
    }

    fn next_token(&mut self) -> crate::Result<Option<Token<'a>>> {
        loop {
            let Some(character) = self.peek() else {
                return Ok(None);
            };
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
            self.token_count += 1;
            if self.token_count > MAX_TOKENS {
                return Err(parse_error(
                    format!("DAT exceeds the {MAX_TOKENS}-token limit"),
                    "document",
                    None,
                    Some(location),
                ));
            }
            let raw = self.input.get(start..self.offset).ok_or_else(|| {
                parse_error("invalid token boundary", "document", None, Some(location))
            })?;
            return Ok(Some(Token {
                kind,
                raw,
                start,
                end: self.offset,
                location,
            }));
        }
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

struct Parser<'a, 'consumer, C: EventConsumer> {
    input: &'a str,
    lexer: Lexer<'a>,
    lookahead: Option<Token<'a>>,
    consumer: &'consumer mut C,
    callbacks_enabled: bool,
    comment_count: usize,
}

impl<'a, 'consumer, C: EventConsumer> Parser<'a, 'consumer, C> {
    const fn new(input: &'a str, consumer: &'consumer mut C) -> Self {
        Self {
            input,
            lexer: Lexer::new(input),
            lookahead: None,
            consumer,
            callbacks_enabled: true,
            comment_count: 0,
        }
    }

    fn emit(&mut self, event: Event) -> Result<(), C::Error> {
        if self.callbacks_enabled {
            self.consumer.consume(event)?;
        }
        Ok(())
    }

    const fn disable_callbacks(&mut self) {
        self.callbacks_enabled = false;
    }

    fn next_form(&mut self) -> Result<Option<Form<'a>>, ParserFailure<C::Error>> {
        self.skip_comments()?;
        if self.peek_token()?.is_none() {
            return Ok(None);
        }
        self.parse_form(0).map(Some)
    }

    fn parse_form(&mut self, depth: usize) -> Result<Form<'a>, ParserFailure<C::Error>> {
        self.skip_comments()?;
        let tag = self.take_word("expected a form name")?;
        self.parse_form_body(tag, depth)
    }

    fn parse_form_body(
        &mut self,
        tag: Token<'a>,
        depth: usize,
    ) -> Result<Form<'a>, ParserFailure<C::Error>> {
        if depth >= MAX_FORM_DEPTH {
            return Err(ParserFailure::Form(parse_error(
                format!("DAT nesting exceeds {MAX_FORM_DEPTH} forms"),
                "document",
                None,
                Some(tag.location),
            )));
        }
        let start = tag.start;
        self.take_left_paren("expected '(' after form name")?;
        let mut items = Vec::new();
        loop {
            self.skip_comments()?;
            match self.peek_token()? {
                Some(Token {
                    kind: TokenKind::RightParen,
                    ..
                }) => {
                    let close = self.take_token()?.ok_or_else(|| {
                        ParserFailure::Form(parse_error(
                            "missing ')'",
                            "document",
                            None,
                            Some(tag.location),
                        ))
                    })?;
                    let raw = self.input.get(start..close.end).ok_or_else(|| {
                        ParserFailure::Form(parse_error(
                            "invalid form token range",
                            "document",
                            None,
                            Some(tag.location),
                        ))
                    })?;
                    return Ok(Form { tag, items, raw });
                }
                None => {
                    return Err(ParserFailure::Form(parse_error(
                        format!("unterminated {} form", tag.value()),
                        tag.value(),
                        None,
                        Some(tag.location),
                    )));
                }
                Some(_) => {}
            }
            let key = self.take_word("expected a keyword or nested form")?;
            self.skip_comments()?;
            if matches!(
                self.peek_token()?.map(|token| &token.kind),
                Some(TokenKind::LeftParen)
            ) {
                items.push(FormItem::Form(self.parse_form_body(key, depth + 1)?));
            } else if key.word_eq("nodump") || key.word_eq("baddump") {
                items.push(FormItem::Flag(key));
            } else if let Some(next) = self.peek_token()? {
                if matches!(next.kind, TokenKind::RightParen) {
                    if is_value_keyword(&tag, &key) {
                        return Err(ParserFailure::Form(parse_error(
                            format!("keyword {} has no value", key.value()),
                            tag.value(),
                            form_record_name(&items),
                            Some(key.location),
                        )));
                    }
                    items.push(FormItem::Flag(key));
                } else if !is_value_keyword(&tag, &key) && is_value_keyword(&tag, next) {
                    items.push(FormItem::Flag(key));
                } else if let Some(value) = self.take_value()? {
                    items.push(FormItem::Field(Field { key, value }));
                } else {
                    return Err(ParserFailure::Form(parse_error(
                        format!("keyword {} has no value", key.value()),
                        tag.value(),
                        form_record_name(&items),
                        Some(key.location),
                    )));
                }
            } else {
                return Err(ParserFailure::Form(parse_error(
                    format!("keyword {} has no value", key.value()),
                    tag.value(),
                    form_record_name(&items),
                    Some(key.location),
                )));
            }
        }
    }

    fn take_word(&mut self, message: &str) -> Result<Token<'a>, ParserFailure<C::Error>> {
        self.skip_comments()?;
        match self.peek_token()? {
            Some(Token {
                kind: TokenKind::Word,
                ..
            }) => self
                .take_token()?
                .ok_or_else(|| ParserFailure::Form(parse_error(message, "document", None, None))),
            token => Err(ParserFailure::Form(parse_error(
                message,
                "document",
                None,
                token.map(|token| token.location),
            ))),
        }
    }

    fn take_value(&mut self) -> Result<Option<Token<'a>>, ParserFailure<C::Error>> {
        if matches!(
            self.peek_token()?.map(|token| &token.kind),
            Some(TokenKind::Word | TokenKind::Quoted(_))
        ) {
            self.take_token()
        } else {
            Ok(None)
        }
    }

    fn take_left_paren(&mut self, message: &str) -> Result<(), ParserFailure<C::Error>> {
        self.skip_comments()?;
        if matches!(
            self.peek_token()?.map(|token| &token.kind),
            Some(TokenKind::LeftParen)
        ) {
            self.take_token()?;
            Ok(())
        } else {
            Err(ParserFailure::Form(parse_error(
                message,
                "document",
                None,
                self.peek_token()?.map(|token| token.location),
            )))
        }
    }

    fn skip_comments(&mut self) -> Result<(), ParserFailure<C::Error>> {
        while matches!(
            self.peek_token()?.map(|token| &token.kind),
            Some(TokenKind::Comment)
        ) {
            self.take_token()?;
        }
        Ok(())
    }

    fn peek_token(&mut self) -> Result<Option<&Token<'a>>, ParserFailure<C::Error>> {
        if self.lookahead.is_none() {
            match self.lexer.next_token() {
                Ok(Some(token)) => {
                    if matches!(token.kind, TokenKind::Comment) {
                        self.comment_count += 1;
                        if self.callbacks_enabled {
                            self.consumer
                                .consume(Event::Comment(Comment {
                                    text: token.raw.to_owned(),
                                    location: token.location,
                                }))
                                .map_err(ParserFailure::Consumer)?;
                        }
                    }
                    self.lookahead = Some(token);
                }
                Ok(None) => {}
                Err(error) => return Err(ParserFailure::Lexical(error)),
            }
        }
        Ok(self.lookahead.as_ref())
    }

    fn take_token(&mut self) -> Result<Option<Token<'a>>, ParserFailure<C::Error>> {
        self.peek_token()?;
        Ok(self.lookahead.take())
    }

    fn drain_lexer(&mut self) -> crate::Result<()> {
        self.lookahead = None;
        while self.lexer.next_token()?.is_some() {}
        Ok(())
    }
}

#[cfg(test)]
mod tests;
