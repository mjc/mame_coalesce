//! Native CMP document, header, and lexical comment reader.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::storage::clrmamepro_fields::{ClrMameProFieldPosition, ClrMameProHeaderField};

use super::{
    ClrMameProComment, ClrMameProDeclaredValue, ClrMameProDocument, ClrMameProForceMerging,
    ClrMameProForceNoDump, ClrMameProForceZipping, ClrMameProHeader, ClrMameProHeaderValue,
    ClrMameProQueryError, positions,
};

type QueryResult<T> = Result<T, ClrMameProQueryError>;

/// Indexed lookup by the document's primary key.
pub(super) const DOCUMENT: &str = "
SELECT CASE WHEN typeof(snapshot_key)='text' THEN snapshot_key END AS snapshot_key,
       CASE WHEN typeof(header_present)='integer' THEN header_present END AS header_present,
       CASE WHEN typeof(comment_count)='integer' THEN comment_count END AS comment_count,
       typeof(snapshot_key)='text' AND typeof(header_present)='integer'
         AND header_present IN (0,1) AND typeof(comment_count)='integer'
         AND comment_count >= 0 AS valid
FROM cmp_documents WHERE snapshot_key=?";

/// Indexed lookup for the optional native header owner and all fifteen scalar fields.
pub(super) const HEADER: &str = "
SELECT CASE WHEN typeof(snapshot_key)='text' THEN snapshot_key END AS snapshot_key,
       CASE WHEN typeof(source_block)='text' THEN source_block END AS source_block,
       CASE WHEN typeof(source_order)='integer' THEN source_order END AS source_order,
       CASE WHEN typeof(source_line)='integer' THEN source_line END AS line,
       CASE WHEN typeof(source_column)='integer' THEN source_column END AS column,
       CASE WHEN typeof(name)='text' THEN name END AS name,
       CASE WHEN typeof(description)='text' THEN description END AS description,
       CASE WHEN typeof(version)='text' THEN version END AS version,
       CASE WHEN typeof(date)='text' THEN date END AS date,
       CASE WHEN typeof(author)='text' THEN author END AS author,
       CASE WHEN typeof(email)='text' THEN email END AS email,
       CASE WHEN typeof(homepage)='text' THEN homepage END AS homepage,
       CASE WHEN typeof(url)='text' THEN url END AS url,
       CASE WHEN typeof(comment)='text' THEN comment END AS comment,
       CASE WHEN typeof(category)='text' THEN category END AS category,
       typeof(snapshot_key)='text' AND typeof(source_block)='text'
         AND typeof(source_order)='integer' AND source_order >= 0
         AND typeof(source_line)='integer' AND source_line > 0
         AND typeof(source_column)='integer' AND source_column > 0
         AND (name IS NULL OR typeof(name)='text')
         AND (description IS NULL OR typeof(description)='text')
         AND (version IS NULL OR typeof(version)='text')
         AND (date IS NULL OR typeof(date)='text')
         AND (author IS NULL OR typeof(author)='text')
         AND (email IS NULL OR typeof(email)='text')
         AND (homepage IS NULL OR typeof(homepage)='text')
         AND (url IS NULL OR typeof(url)='text')
         AND (comment IS NULL OR typeof(comment)='text')
         AND (category IS NULL OR typeof(category)='text') AS valid
FROM cmp_header_facts WHERE snapshot_key=?";

/// Indexed lookup for stored directives and their database-derived effective values.
pub(super) const DIRECTIVES: &str = "
SELECT CASE WHEN typeof(snapshot_key)='text' THEN snapshot_key END AS snapshot_key,
       CASE WHEN typeof(header_definition)='text' THEN header_definition END AS header_definition,
       CASE WHEN typeof(forcemerging)='text' THEN forcemerging END AS forcemerging,
       CASE WHEN typeof(forcezipping)='text' THEN forcezipping END AS forcezipping,
       CASE WHEN typeof(forcepacking)='text' THEN forcepacking END AS forcepacking,
       CASE WHEN typeof(forcenodump)='text' THEN forcenodump END AS forcenodump,
       CASE WHEN typeof(forcemerging_effective)='text' THEN forcemerging_effective END AS forcemerging_effective,
       CASE WHEN typeof(forcezipping_effective)='text' THEN forcezipping_effective END AS forcezipping_effective,
       CASE WHEN typeof(forcenodump_effective)='text' THEN forcenodump_effective END AS forcenodump_effective,
       typeof(snapshot_key)='text'
         AND (header_definition IS NULL OR typeof(header_definition)='text')
         AND (forcemerging IS NULL OR typeof(forcemerging)='text')
         AND (forcezipping IS NULL OR typeof(forcezipping)='text')
         AND (forcepacking IS NULL OR typeof(forcepacking)='text')
         AND (forcenodump IS NULL OR typeof(forcenodump)='text')
         AND (forcemerging_effective IS NULL OR typeof(forcemerging_effective)='text')
         AND (forcezipping_effective IS NULL OR typeof(forcezipping_effective)='text')
         AND (forcenodump_effective IS NULL OR typeof(forcenodump_effective)='text') AS valid
FROM cmp_header_directives WHERE snapshot_key=?";

/// Indexed lookup by snapshot owner, ordered by each field's source token ordinal.
pub(super) const HEADER_FIELD_POSITIONS: &str = "
SELECT CASE WHEN typeof(snapshot_key)='text' THEN snapshot_key END AS snapshot_key,
       CASE WHEN typeof(field_kind)='integer' THEN field_kind END AS field_kind,
       CASE WHEN typeof(source_field)='text' THEN source_field END AS source_field,
       CASE WHEN typeof(source_order)='integer' THEN source_order END AS source_order,
       CASE WHEN typeof(is_quoted)='integer' THEN is_quoted END AS is_quoted,
       CASE WHEN typeof(source_line)='integer' THEN source_line END AS line,
       CASE WHEN typeof(source_column)='integer' THEN source_column END AS column,
       typeof(snapshot_key)='text' AND typeof(field_kind)='integer'
         AND typeof(source_field)='text' AND typeof(source_order)='integer'
         AND source_order >= 0 AND typeof(is_quoted)='integer' AND is_quoted IN (0,1)
         AND typeof(source_line)='integer' AND source_line > 0
         AND typeof(source_column)='integer' AND source_column > 0 AS valid
FROM cmp_header_field_positions WHERE snapshot_key=? ORDER BY source_order";

/// Indexed lookup by snapshot owner, in native lexical comment order.
pub(super) const COMMENTS: &str = "
SELECT CASE WHEN typeof(snapshot_key)='text' THEN snapshot_key END AS snapshot_key,
       CASE WHEN typeof(comment_order)='integer' THEN comment_order END AS comment_order,
       CASE WHEN typeof(text)='text' THEN text END AS text,
       CASE WHEN typeof(source_line)='integer' THEN source_line END AS line,
       CASE WHEN typeof(source_column)='integer' THEN source_column END AS column,
       typeof(snapshot_key)='text' AND typeof(comment_order)='integer'
         AND comment_order >= 0 AND typeof(text)='text'
         AND typeof(source_line)='integer' AND source_line > 0
         AND typeof(source_column)='integer' AND source_column > 0 AS valid
FROM cmp_comments WHERE snapshot_key=? ORDER BY comment_order";

/// Production owner-key statements and the table each plan must search.
#[cfg(test)]
pub(super) fn plan_statements() -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    vec![
        ("document", DOCUMENT, vec!["cmp_documents"]),
        ("header", HEADER, vec!["cmp_header_facts"]),
        ("directives", DIRECTIVES, vec!["cmp_header_directives"]),
        (
            "header field positions and orphan closure",
            HEADER_FIELD_POSITIONS,
            vec!["cmp_header_field_positions"],
        ),
        ("comments", COMMENTS, vec!["cmp_comments"]),
    ]
}

#[derive(QueryableByName)]
struct DocumentRow {
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_key: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    header_present: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    comment_count: Option<i64>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct HeaderRow {
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    source_block: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    column: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    email: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    category: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct DirectivesRow {
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_definition: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcemerging: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcezipping: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcepacking: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcemerging_effective: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcezipping_effective: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump_effective: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct PositionRow {
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_key: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    field_kind: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    source_field: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    is_quoted: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    column: Option<i64>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct CommentRow {
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_key: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    comment_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    column: Option<i64>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

/// Load the native document, rejecting incomplete or malformed persisted ownership.
pub(super) fn load(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> QueryResult<ClrMameProDocument> {
    let row = sql_query(DOCUMENT)
        .bind::<Text, _>(snapshot)
        .get_result::<DocumentRow>(connection)
        .optional()?
        .ok_or(ClrMameProQueryError::InvalidMetadata(0))?;
    if row.valid != 1 || row.snapshot_key.as_deref() != Some(snapshot) {
        return Err(ClrMameProQueryError::InvalidMetadata(0));
    }
    let header_present = match row.header_present {
        Some(0) => false,
        Some(1) => true,
        _ => return Err(ClrMameProQueryError::InvalidMetadata(0)),
    };
    let comment_count = row
        .comment_count
        .filter(|count| *count >= 0)
        .ok_or(ClrMameProQueryError::InvalidMetadata(0))?;

    let header = sql_query(HEADER)
        .bind::<Text, _>(snapshot)
        .get_result::<HeaderRow>(connection)
        .optional()?;
    let directives = sql_query(DIRECTIVES)
        .bind::<Text, _>(snapshot)
        .get_result::<DirectivesRow>(connection)
        .optional()?;
    let position_rows = sql_query(HEADER_FIELD_POSITIONS)
        .bind::<Text, _>(snapshot)
        .load::<PositionRow>(connection)?;

    let header = match (header_present, header, directives) {
        (false, None, None) if position_rows.is_empty() => None,
        (true, Some(header), Some(directives)) => {
            Some(load_header(header, directives, position_rows, snapshot)?)
        }
        _ => return Err(ClrMameProQueryError::InvalidMetadata(0)),
    };

    let comments = sql_query(COMMENTS)
        .bind::<Text, _>(snapshot)
        .load::<CommentRow>(connection)?
        .into_iter()
        .enumerate()
        .map(|(index, row)| load_comment(row, index, snapshot))
        .collect::<QueryResult<Vec<_>>>()?;
    if i64::try_from(comments.len()).ok() != Some(comment_count) {
        return Err(ClrMameProQueryError::InvalidMetadata(0));
    }

    Ok(ClrMameProDocument {
        header_present,
        comment_count,
        comments,
        header,
    })
}

fn load_header(
    row: HeaderRow,
    directives: DirectivesRow,
    position_rows: Vec<PositionRow>,
    snapshot: &str,
) -> QueryResult<ClrMameProHeader> {
    if row.valid != 1
        || row.snapshot_key.as_deref() != Some(snapshot)
        || !row
            .source_block
            .as_deref()
            .is_some_and(|block| block.eq_ignore_ascii_case("clrmamepro"))
        || directives.valid != 1
        || directives.snapshot_key.as_deref() != Some(snapshot)
    {
        return Err(ClrMameProQueryError::InvalidMetadata(0));
    }
    let source_block = row
        .source_block
        .ok_or(ClrMameProQueryError::InvalidMetadata(0))?;
    let source_order = positions::order(
        row.source_order
            .ok_or(ClrMameProQueryError::InvalidMetadata(0))?,
        0,
    )?;
    let location = positions::location(
        row.line.ok_or(ClrMameProQueryError::InvalidMetadata(0))?,
        row.column.ok_or(ClrMameProQueryError::InvalidMetadata(0))?,
        0,
    )?;

    let declarations = [
        (ClrMameProHeaderField::Name, row.name),
        (ClrMameProHeaderField::Description, row.description),
        (ClrMameProHeaderField::Version, row.version),
        (ClrMameProHeaderField::Date, row.date),
        (ClrMameProHeaderField::Author, row.author),
        (ClrMameProHeaderField::Email, row.email),
        (ClrMameProHeaderField::Homepage, row.homepage),
        (ClrMameProHeaderField::Url, row.url),
        (ClrMameProHeaderField::Comment, row.comment),
        (ClrMameProHeaderField::Category, row.category),
        (
            ClrMameProHeaderField::HeaderDefinition,
            directives.header_definition,
        ),
        (ClrMameProHeaderField::ForceMerging, directives.forcemerging),
        (ClrMameProHeaderField::ForceZipping, directives.forcezipping),
        (ClrMameProHeaderField::ForcePacking, directives.forcepacking),
        (ClrMameProHeaderField::ForceNoDump, directives.forcenodump),
    ];
    let mut positions = load_positions(position_rows, snapshot)?;
    let fields = load_fields(declarations, &mut positions)?;

    Ok(ClrMameProHeader {
        source_block,
        source_order,
        location,
        fields,
        forcemerging_effective: match directives.forcemerging_effective.as_deref() {
            Some("none") => Some(ClrMameProForceMerging::None),
            Some("split") => Some(ClrMameProForceMerging::Split),
            Some("full") => Some(ClrMameProForceMerging::Full),
            _ => None,
        },
        forcezipping_effective: match directives.forcezipping_effective.as_deref() {
            Some("zip") => Some(ClrMameProForceZipping::Zip),
            Some("unzip") => Some(ClrMameProForceZipping::Unzip),
            _ => None,
        },
        forcenodump_effective: match directives.forcenodump_effective.as_deref() {
            Some("obsolete") => Some(ClrMameProForceNoDump::Obsolete),
            Some("required") => Some(ClrMameProForceNoDump::Required),
            Some("ignore") => Some(ClrMameProForceNoDump::Ignore),
            _ => None,
        },
    })
}

fn load_positions(
    rows: Vec<PositionRow>,
    snapshot: &str,
) -> QueryResult<BTreeMap<i64, ClrMameProFieldPosition>> {
    let mut positions = BTreeMap::new();
    let mut source_orders = BTreeSet::new();
    for row in rows {
        if row.valid != 1 || row.snapshot_key.as_deref() != Some(snapshot) {
            return Err(ClrMameProQueryError::InvalidPositions(0));
        }
        let code = row
            .field_kind
            .ok_or(ClrMameProQueryError::InvalidPositions(0))?;
        let field = ClrMameProHeaderField::from_code(code)
            .ok_or(ClrMameProQueryError::InvalidPositions(0))?;
        let source_field = row
            .source_field
            .ok_or(ClrMameProQueryError::InvalidPositions(0))?;
        if !source_field.eq_ignore_ascii_case(field.keyword()) {
            return Err(ClrMameProQueryError::InvalidPositions(0));
        }
        let source_order = positions::order(
            row.source_order
                .ok_or(ClrMameProQueryError::InvalidPositions(0))?,
            0,
        )?;
        if !source_orders.insert(source_order) {
            return Err(ClrMameProQueryError::InvalidPositions(0));
        }
        let is_quoted = match row.is_quoted {
            Some(0) => false,
            Some(1) => true,
            _ => return Err(ClrMameProQueryError::InvalidPositions(0)),
        };
        let position = ClrMameProFieldPosition {
            source_field,
            source_order,
            is_quoted,
            location: positions::location(
                row.line.ok_or(ClrMameProQueryError::InvalidPositions(0))?,
                row.column
                    .ok_or(ClrMameProQueryError::InvalidPositions(0))?,
                0,
            )?,
        };
        if positions.insert(code, position).is_some() {
            return Err(ClrMameProQueryError::InvalidPositions(0));
        }
    }
    Ok(positions)
}

fn load_fields(
    declarations: [(ClrMameProHeaderField, Option<String>); 15],
    positions: &mut BTreeMap<i64, ClrMameProFieldPosition>,
) -> QueryResult<Vec<ClrMameProHeaderValue>> {
    let mut fields = Vec::new();
    for (field, value) in declarations {
        match (value, positions.remove(&(field as i64))) {
            (None, None) => {}
            (Some(value), Some(position)) => fields.push((
                position.source_order,
                ClrMameProHeaderValue {
                    field,
                    value: ClrMameProDeclaredValue { value, position },
                },
            )),
            _ => return Err(ClrMameProQueryError::InvalidPositions(0)),
        }
    }
    if !positions.is_empty() {
        return Err(ClrMameProQueryError::InvalidPositions(0));
    }
    fields.sort_by_key(|(source_order, _)| *source_order);
    Ok(fields.into_iter().map(|(_, field)| field).collect())
}

fn load_comment(row: CommentRow, index: usize, snapshot: &str) -> QueryResult<ClrMameProComment> {
    if row.valid != 1 || row.snapshot_key.as_deref() != Some(snapshot) {
        return Err(ClrMameProQueryError::InvalidMetadata(0));
    }
    let comment_order = row
        .comment_order
        .filter(|order| *order >= 0)
        .ok_or(ClrMameProQueryError::InvalidMetadata(0))?;
    if usize::try_from(comment_order).ok() != Some(index) {
        return Err(ClrMameProQueryError::InvalidMetadata(0));
    }
    Ok(ClrMameProComment {
        comment_order,
        text: row.text.ok_or(ClrMameProQueryError::InvalidMetadata(0))?,
        location: positions::location(
            row.line.ok_or(ClrMameProQueryError::InvalidMetadata(0))?,
            row.column.ok_or(ClrMameProQueryError::InvalidMetadata(0))?,
            0,
        )?,
    })
}
