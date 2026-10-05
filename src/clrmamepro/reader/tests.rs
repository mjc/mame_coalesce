#![allow(clippy::expect_used)]

use super::{Comment, EofSeal, Event, EventConsumer, read_with};

#[derive(Default)]
struct Sink {
    comments: Vec<Comment>,
    sets: usize,
}

impl EventConsumer for Sink {
    type Error = crate::Error;

    fn consume(&mut self, event: Event) -> crate::Result<()> {
        match event {
            Event::Comment(comment) => self.comments.push(comment),
            Event::Set(_) => self.sets += 1,
            Event::Header(..) | Event::Extension(_) => {}
        }
        Ok(())
    }
}

fn parse(source: &str) -> crate::Result<(EofSeal, Sink)> {
    let mut sink = Sink::default();
    let seal = read_with(source.as_bytes(), &mut sink)?;
    Ok((seal, sink))
}

fn record_kind(error: &crate::Error) -> Option<&str> {
    match error {
        crate::Error::CatalogParse { record_kind, .. } => record_kind.as_deref(),
        _ => None,
    }
}

#[test]
fn comments_are_streamed_once_and_seal_only_follows_eof() -> crate::Result<()> {
    let (seal, sink) = parse(
        r#"; before
clrmamepro ( name "header ; inside quoted value" )
game ( name one ; between keys
 rom ; nested
 ( name one.bin ) ) ; after
"#,
    )?;

    assert_eq!(seal.comment_count(), 4);
    assert_eq!(sink.comments.len(), 4);
    assert!(seal.header_present());
    assert_eq!(seal.set_count(), 1);
    assert_eq!(sink.sets, 1);
    Ok(())
}

#[test]
fn deferred_error_priority_is_duplicate_then_header_then_set_then_no_sets() {
    let duplicate = "\nclrmamepro ( name a )\nclrmamepro ( name c ) game ( name x name y )";
    let error = read_with(duplicate.as_bytes(), &mut Sink::default())
        .expect_err("duplicate header must outrank later semantic failures");
    assert!(error.to_string().contains("multiple clrmamepro headers"));
    assert!(matches!(
        error,
        crate::Error::CatalogParse {
            line: Some(2),
            column: Some(1),
            ..
        }
    ));

    let header_first = "clrmamepro ( name a name b ) game ( name x name y )";
    let error = read_with(header_first.as_bytes(), &mut Sink::default())
        .expect_err("invalid header must be rejected");
    assert!(error.to_string().contains("duplicate name field"));
    assert_eq!(record_kind(&error), Some("clrmamepro"));

    let no_sets = "clrmamepro ( name a )";
    let error = read_with(no_sets.as_bytes(), &mut Sink::default())
        .expect_err("document without sets must be rejected");
    assert!(error.to_string().contains("no set/game records"));
}

#[test]
fn later_higher_priority_semantic_errors_win_and_disable_callbacks() {
    let source = "game ( name broken name duplicate ) clrmamepro ( name a name b )";
    let mut sink = Sink::default();
    let error = read_with(source.as_bytes(), &mut sink)
        .expect_err("later header error must outrank earlier set error");
    assert!(error.to_string().contains("duplicate name field"));
    assert_eq!(record_kind(&error), Some("clrmamepro"));
    assert_eq!(sink.sets, 0);

    let source =
        "game ( name first ) clrmamepro ( name a ) clrmamepro ( name b ) game ( name last )";
    let mut sink = Sink::default();
    let error = read_with(source.as_bytes(), &mut sink)
        .expect_err("duplicate header must outrank later events");
    assert!(error.to_string().contains("multiple clrmamepro headers"));
    assert_eq!(
        sink.sets, 1,
        "callbacks stop after the duplicate header is known"
    );
}

#[test]
fn later_form_failure_precedes_deferred_semantic_errors() {
    let source = "clrmamepro ( name a name b ) game ( name valid ) game ( name";
    let error = read_with(source.as_bytes(), &mut Sink::default())
        .expect_err("form syntax error must outrank deferred semantic errors");
    assert!(error.to_string().contains("keyword name has no value"));
    assert_eq!(record_kind(&error), Some("game"));
}

#[test]
fn parser_failure_drains_for_later_lexical_failure_and_disables_callbacks() {
    let mut sink = Sink::default();
    let source = "game ( name first ) game ( name ) game ( name later ) \"unterminated";
    let error = read_with(source.as_bytes(), &mut sink)
        .expect_err("lexer error after malformed form must be returned");
    assert!(error.to_string().contains("unterminated quoted value"));
    assert_eq!(
        sink.sets, 1,
        "events after a known parser error must be disabled"
    );
}

#[test]
fn token_budget_counts_comments_and_accepts_exact_limit() -> crate::Result<()> {
    let form = "game ( name x ) ";
    let mut source = String::with_capacity(form.len() * 200_000);
    for _ in 0..200_000 {
        source.push_str(form);
    }
    let seal = read_with(source.as_bytes(), &mut Sink::default())?;
    assert_eq!(seal.set_count(), 200_000);

    source.push_str("; one extra token\n");
    let error = read_with(source.as_bytes(), &mut Sink::default())
        .expect_err("comment beyond the global token budget must fail");
    assert!(error.to_string().contains("1000000-token limit"));
    Ok(())
}

#[test]
fn form_depth_accepts_128_and_rejects_129_forms() -> crate::Result<()> {
    fn nested(total_forms: usize) -> String {
        let mut source = String::from("game ( name depth ");
        for _ in 1..total_forms {
            source.push_str("x ( ");
        }
        for _ in 1..total_forms {
            source.push_str(") ");
        }
        source.push(')');
        source
    }

    let _ = read_with(nested(128).as_bytes(), &mut Sink::default())?;
    let error = read_with(nested(129).as_bytes(), &mut Sink::default())
        .expect_err("129 nested forms must exceed the existing depth limit");
    assert!(error.to_string().contains("nesting exceeds 128 forms"));
    Ok(())
}
