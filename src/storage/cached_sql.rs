//! Cached statements for the finite, repository-owned native import SQL.
//!
//! SQL is keyed by its text, never by the Rust type of its bind values. Static
//! statements and bounded trusted generated templates use anonymous `?`
//! placeholders. Batch INSERT parsing recognizes quoted SQL constants and
//! ignores their `?`.

use std::{marker::PhantomData, str::Split};

use diesel::{
    RunQueryDsl, SqliteConnection,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    serialize::ToSql,
    sql_types::{HasSqlType, Untyped},
    sqlite::Sqlite,
};

pub(super) struct CachedSql<B = ()> {
    sql: &'static str,
    bindings: B,
}

const MAX_BATCH_BIND_PARAMETERS: usize = 999;
const MAX_BATCH_ROWS: usize = 256;
const MAX_BATCH_BYTES: usize = 1024 * 1024;
const MAX_QUEUED_ROWS: usize = 16 * 1024;
const MAX_QUEUED_BYTES: usize = 16 * 1024 * 1024;

pub(super) const fn cached_sql(sql: &'static str) -> CachedSql {
    CachedSql { sql, bindings: () }
}

/// Build a query from a trusted, repository-generated SQL template.
///
/// The bind count must match the placeholders. Callers must parameterize
/// values and bound both template arity and the set of SQL texts admitted to
/// Diesel's prepared statement cache. Do not pass user-provided SQL.
pub(super) const fn cached_generated_sql(
    sql: String,
    bindings: Vec<OwnedBinding>,
) -> BatchQuery<'static> {
    BatchQuery {
        sql,
        bindings: BatchBindings::Owned(bindings),
        cacheable: true,
    }
}

/// Build a generated query whose text and binary values remain borrowed until
/// Diesel has encoded the statement.
pub(super) const fn cached_generated_sql_borrowed(
    sql: String,
    bindings: Vec<BorrowedBinding<'_>>,
) -> BatchQuery<'_> {
    BatchQuery {
        sql,
        bindings: BatchBindings::Borrowed(bindings),
        cacheable: true,
    }
}

impl<B> CachedSql<B> {
    pub(super) fn bind<ST, V>(self, value: V) -> CachedSql<(B, Binding<ST, V>)> {
        CachedSql {
            sql: self.sql,
            bindings: (
                self.bindings,
                Binding {
                    value,
                    sql_type: PhantomData,
                },
            ),
        }
    }
}

impl<B: OwnedBindings> CachedSql<B> {
    /// Add one owned row to a batch. SQL is restricted to INSERT statements
    /// with one VALUES tuple so constants and column order remain explicit.
    pub(super) fn enqueue(
        self,
        batch: &mut InsertBatch,
        conn: &mut SqliteConnection,
        phase: InsertPhase,
    ) -> diesel::QueryResult<()> {
        let bindings = self.bindings.into_owned();
        batch.push(self.sql, bindings, phase, conn)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum InsertPhase {
    Parents,
    Children,
}

/// Owned, dependency-ordered rows ready for multi-row VALUES execution.
/// Callers bound retained memory by queueing one input chunk at a time.
#[derive(Default)]
pub(super) struct InsertBatch {
    groups: Vec<InsertGroup>,
    rows: usize,
    bytes: usize,
}

impl InsertBatch {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn flush(&mut self, conn: &mut SqliteConnection) -> diesel::QueryResult<()> {
        self.groups.sort_by_key(|group| group.phase);
        for group in &mut self.groups {
            group.execute(conn)?;
        }
        self.rows = 0;
        self.bytes = 0;
        Ok(())
    }

    fn push(
        &mut self,
        sql: &str,
        bindings: Vec<OwnedBinding>,
        phase: InsertPhase,
        conn: &mut SqliteConnection,
    ) -> diesel::QueryResult<()> {
        let mut group_index = self
            .groups
            .iter()
            .position(|group| group.phase == phase && group.shape.sql == sql);
        let new_shape = if group_index.is_none() {
            Some(InsertShape::parse(sql)?)
        } else {
            None
        };
        let shape_details = group_index
            .map(|index| {
                let shape = &self.groups[index].shape;
                (
                    shape.parameter_count,
                    shape.tuple_sql.len(),
                    shape.prefix.len(),
                )
            })
            .or_else(|| {
                new_shape.as_ref().map(|shape| {
                    (
                        shape.parameter_count,
                        shape.tuple_sql.len(),
                        shape.prefix.len(),
                    )
                })
            });
        let Some((parameter_count, tuple_bytes, prefix_bytes)) = shape_details else {
            return Err(query_builder_error("missing INSERT shape"));
        };
        if bindings.len() != parameter_count {
            return Err(query_builder_error(
                "binding count does not match INSERT tuple",
            ));
        }
        let row_bytes =
            bindings.iter().map(OwnedBinding::size).sum::<usize>() + tuple_bytes + prefix_bytes;
        if self.rows > 0
            && (self.rows >= MAX_QUEUED_ROWS
                || self.bytes.saturating_add(row_bytes) > MAX_QUEUED_BYTES)
        {
            self.flush(conn)?;
            // `flush` orders groups by dependency phase, so any index found
            // above may now refer to another SQL shape.
            group_index = self
                .groups
                .iter()
                .position(|group| group.phase == phase && group.shape.sql == sql);
        }
        self.rows += 1;
        self.bytes = self.bytes.saturating_add(row_bytes);
        if let Some(index) = group_index {
            let group = &mut self.groups[index];
            group.rows.push(bindings);
        } else {
            let Some(shape) = new_shape else {
                return Err(query_builder_error("new INSERT shape was not parsed"));
            };
            self.groups.push(InsertGroup {
                phase,
                shape,
                rows: vec![bindings],
            });
        }
        if self.rows >= MAX_QUEUED_ROWS || self.bytes >= MAX_QUEUED_BYTES {
            self.flush(conn)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InsertShape {
    sql: String,
    prefix: String,
    tuple: Vec<String>,
    tuple_sql: String,
    parameter_count: usize,
}

impl InsertShape {
    fn parse(sql: &str) -> diesel::QueryResult<Self> {
        let invalid = || query_builder_error("unsupported cached INSERT shape");
        let trimmed = sql.trim();
        if !trimmed.starts_with("INSERT INTO ") || has_unquoted(trimmed, ';') {
            return Err(invalid());
        }
        let upper = trimmed.to_ascii_uppercase();
        let values_at = upper.find(" VALUES ").ok_or_else(invalid)?;
        let prefix = &trimmed[..values_at];
        let values = trimmed[values_at + " VALUES ".len()..].trim();
        let tuple_text = values.strip_prefix('(').ok_or_else(invalid)?;
        let end = find_tuple_end(tuple_text).ok_or_else(invalid)?;
        if !tuple_text[end + 1..].trim().is_empty() {
            return Err(invalid());
        }
        let tuple = split_sql_values(&tuple_text[..end]).ok_or_else(invalid)?;
        if tuple.is_empty() || tuple.iter().any(|value| !valid_tuple_value(value)) {
            return Err(invalid());
        }
        let columns = prefix
            .rfind('(')
            .and_then(|start| {
                prefix[start + 1..]
                    .find(')')
                    .map(|end| &prefix[start + 1..start + 1 + end])
            })
            .ok_or_else(invalid)?;
        if columns.split(',').count() != tuple.len() {
            return Err(invalid());
        }
        let parameter_count = tuple.iter().filter(|value| value.as_str() == "?").count();
        let tuple_sql = tuple.join(",");
        Ok(Self {
            sql: trimmed.to_owned(),
            prefix: prefix.to_owned(),
            tuple,
            tuple_sql,
            parameter_count,
        })
    }
}

fn valid_tuple_value(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && (value == "?"
            || matches!(value, "NULL" | "TRUE" | "FALSE")
            || value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'+' | b'-'))
            || (value.starts_with('\'') && value.ends_with('\'') && quoted_literal_is_valid(value)))
}

fn quoted_literal_is_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 1;
    while index + 1 < bytes.len() {
        if bytes[index] == b'\'' {
            if bytes.get(index + 1) != Some(&b'\'') {
                return false;
            }
            index += 1;
        }
        index += 1;
    }
    true
}

fn has_unquoted(sql: &str, needle: char) -> bool {
    let mut quoted = false;
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' if quoted && chars.peek() == Some(&'\'') => {
                chars.next();
            }
            '\'' => quoted = !quoted,
            ch if !quoted && ch == needle => return true,
            _ => {}
        }
    }
    false
}

fn find_tuple_end(sql: &str) -> Option<usize> {
    let mut quoted = false;
    let mut chars = sql.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\'' if quoted && chars.peek().is_some_and(|(_, next)| *next == '\'') => {
                chars.next();
            }
            '\'' => quoted = !quoted,
            ')' if !quoted => return Some(index),
            _ => {}
        }
    }
    None
}

fn split_sql_values(sql: &str) -> Option<Vec<String>> {
    let mut values = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut chars = sql.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\'' if quoted && chars.peek().is_some_and(|(_, next)| *next == '\'') => {
                chars.next();
            }
            '\'' => quoted = !quoted,
            ',' if !quoted => {
                values.push(sql[start..index].trim().to_owned());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if quoted {
        return None;
    }
    values.push(sql[start..].trim().to_owned());
    Some(values)
}

struct InsertGroup {
    phase: InsertPhase,
    shape: InsertShape,
    rows: Vec<Vec<OwnedBinding>>,
}

impl InsertGroup {
    fn execute(&mut self, conn: &mut SqliteConnection) -> diesel::QueryResult<()> {
        let row_parameters = self.shape.parameter_count;
        let row_sql_bytes = self.shape.tuple_sql.len();
        let rows = std::mem::take(&mut self.rows);
        let mut index = 0;
        while index < rows.len() {
            let mut end = index;
            let mut bytes = self.shape.prefix.len() + 16;
            while end < rows.len() && end - index < MAX_BATCH_ROWS {
                let next_bytes = row_sql_bytes
                    .saturating_add(rows[end].iter().map(OwnedBinding::size).sum::<usize>());
                if end > index
                    && (row_parameters.saturating_mul(end + 1 - index) > MAX_BATCH_BIND_PARAMETERS
                        || bytes.saturating_add(next_bytes) > MAX_BATCH_BYTES)
                {
                    break;
                }
                bytes = bytes.saturating_add(next_bytes);
                end += 1;
            }
            let mut sql = self.shape.prefix.clone();
            sql.push_str(" VALUES ");
            for row in &rows[index..end] {
                if !sql.ends_with(" VALUES ") {
                    sql.push(',');
                }
                sql.push('(');
                sql.push_str(&self.shape.tuple_sql);
                sql.push(')');
                debug_assert_eq!(row.len(), row_parameters);
            }
            // Cache powers of two plus the maximum row count for this shape.
            // Arbitrary tails still execute normally but cannot grow the cache
            // once for every distinct VALUES tuple count.
            let rows_in_statement = end - index;
            let max_rows = MAX_BATCH_ROWS.min(MAX_BATCH_BIND_PARAMETERS / row_parameters.max(1));
            let cacheable = rows_in_statement.is_power_of_two() || rows_in_statement == max_rows;
            BatchQuery {
                sql,
                bindings: BatchBindings::OwnedRows {
                    rows: &rows[index..end],
                    parameters_per_row: row_parameters,
                },
                cacheable,
            }
            .run(conn)?;
            index = end;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) enum OwnedBinding {
    BigInt(i64),
    Text(String),
    NullableText(Option<String>),
    NullableBigInt(Option<i64>),
    NullableBool(Option<bool>),
    NullableBinary(Option<Vec<u8>>),
    Bool(bool),
    Binary(Vec<u8>),
}

/// Borrowed scalar values supported by generated, immediately executed SQL.
pub(super) enum BorrowedBinding<'a> {
    BigInt(i64),
    Text(&'a str),
    Binary(&'a [u8]),
}

enum BatchBindings<'a> {
    Owned(Vec<OwnedBinding>),
    Borrowed(Vec<BorrowedBinding<'a>>),
    OwnedRows {
        rows: &'a [Vec<OwnedBinding>],
        parameters_per_row: usize,
    },
}

impl BatchBindings<'_> {
    const fn len(&self) -> usize {
        match self {
            Self::Owned(bindings) => bindings.len(),
            Self::Borrowed(bindings) => bindings.len(),
            Self::OwnedRows {
                rows,
                parameters_per_row,
            } => rows.len().saturating_mul(*parameters_per_row),
        }
    }

    fn push_bind<'b>(
        &'b self,
        index: usize,
        pass: &mut AstPass<'_, 'b, Sqlite>,
    ) -> diesel::QueryResult<()> {
        match self {
            Self::Owned(bindings) => bindings
                .get(index)
                .ok_or_else(|| query_builder_error("more SQL placeholders than batch bindings"))?
                .push_bind(pass),
            Self::Borrowed(bindings) => bindings
                .get(index)
                .ok_or_else(|| query_builder_error("more SQL placeholders than batch bindings"))?
                .push_bind(pass),
            Self::OwnedRows {
                rows,
                parameters_per_row,
            } => {
                if *parameters_per_row == 0 {
                    return Err(query_builder_error("missing generated SQL binding"));
                }
                rows.get(index / *parameters_per_row)
                    .and_then(|row| row.get(index % *parameters_per_row))
                    .ok_or_else(|| {
                        query_builder_error("more SQL placeholders than batch bindings")
                    })?
                    .push_bind(pass)
            }
        }
    }
}

impl BorrowedBinding<'_> {
    fn push_bind<'b>(&'b self, pass: &mut AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        match self {
            Self::BigInt(value) => pass.push_bind_param::<diesel::sql_types::BigInt, _>(value),
            Self::Text(value) => pass.push_bind_param::<diesel::sql_types::Text, _>(value),
            Self::Binary(value) => pass.push_bind_param::<diesel::sql_types::Binary, _>(value),
        }
    }
}

impl OwnedBinding {
    fn size(&self) -> usize {
        match self {
            Self::BigInt(_) | Self::NullableBigInt(_) => std::mem::size_of::<i64>(),
            Self::Text(value) => value.len(),
            Self::NullableText(value) => value.as_ref().map_or(0, String::len),
            Self::NullableBool(_) | Self::Bool(_) => 1,
            Self::NullableBinary(value) => value.as_ref().map_or(0, Vec::len),
            Self::Binary(value) => value.len(),
        }
    }

    fn push_bind<'b>(&'b self, pass: &mut AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        match self {
            Self::BigInt(value) => pass.push_bind_param::<diesel::sql_types::BigInt, _>(value),
            Self::Text(value) => pass.push_bind_param::<diesel::sql_types::Text, _>(value),
            Self::NullableText(value) => pass
                .push_bind_param::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(value),
            Self::NullableBigInt(value) => pass.push_bind_param::<diesel::sql_types::Nullable<
                diesel::sql_types::BigInt,
            >, _>(value),
            Self::NullableBool(value) => pass
                .push_bind_param::<diesel::sql_types::Nullable<diesel::sql_types::Bool>, _>(value),
            Self::NullableBinary(value) => pass.push_bind_param::<diesel::sql_types::Nullable<
                diesel::sql_types::Binary,
            >, _>(value),
            Self::Bool(value) => pass.push_bind_param::<diesel::sql_types::Bool, _>(value),
            Self::Binary(value) => pass.push_bind_param::<diesel::sql_types::Binary, _>(value),
        }
    }
}

pub(super) trait IntoOwnedBinding<ST> {
    fn into_owned_binding(self) -> OwnedBinding;
}

impl IntoOwnedBinding<diesel::sql_types::BigInt> for i64 {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::BigInt(self)
    }
}

impl IntoOwnedBinding<diesel::sql_types::Text> for &str {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::Text(self.to_owned())
    }
}

impl IntoOwnedBinding<diesel::sql_types::Text> for &String {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::Text(self.clone())
    }
}

impl IntoOwnedBinding<diesel::sql_types::Text> for String {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::Text(self)
    }
}

impl IntoOwnedBinding<diesel::sql_types::Nullable<diesel::sql_types::Text>> for Option<&str> {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::NullableText(self.map(str::to_owned))
    }
}

impl IntoOwnedBinding<diesel::sql_types::Nullable<diesel::sql_types::Text>> for Option<String> {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::NullableText(self)
    }
}

impl IntoOwnedBinding<diesel::sql_types::Nullable<diesel::sql_types::Text>> for Option<&String> {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::NullableText(self.cloned())
    }
}

impl IntoOwnedBinding<diesel::sql_types::Bool> for bool {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::Bool(self)
    }
}

impl IntoOwnedBinding<diesel::sql_types::Nullable<diesel::sql_types::BigInt>> for Option<i64> {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::NullableBigInt(self)
    }
}

impl IntoOwnedBinding<diesel::sql_types::Nullable<diesel::sql_types::Bool>> for Option<bool> {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::NullableBool(self)
    }
}

impl IntoOwnedBinding<diesel::sql_types::Binary> for Vec<u8> {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::Binary(self)
    }
}

impl IntoOwnedBinding<diesel::sql_types::Nullable<diesel::sql_types::Binary>> for Option<Vec<u8>> {
    fn into_owned_binding(self) -> OwnedBinding {
        OwnedBinding::NullableBinary(self)
    }
}

pub(super) trait OwnedBindings {
    fn into_owned(self) -> Vec<OwnedBinding>;
}

impl OwnedBindings for () {
    fn into_owned(self) -> Vec<OwnedBinding> {
        Vec::new()
    }
}

impl<B, ST, V> OwnedBindings for (B, Binding<ST, V>)
where
    B: OwnedBindings,
    V: IntoOwnedBinding<ST>,
{
    fn into_owned(self) -> Vec<OwnedBinding> {
        let mut bindings = self.0.into_owned();
        bindings.push(self.1.value.into_owned_binding());
        bindings
    }
}

pub(super) struct BatchQuery<'a> {
    sql: String,
    bindings: BatchBindings<'a>,
    cacheable: bool,
}

impl BatchQuery<'_> {
    fn run(self, conn: &mut SqliteConnection) -> diesel::QueryResult<()> {
        <Self as diesel::query_dsl::methods::ExecuteDsl<SqliteConnection>>::execute(self, conn)?;
        Ok(())
    }
}

impl QueryId for BatchQuery<'_> {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl Query for BatchQuery<'_> {
    type SqlType = Untyped;
}

impl RunQueryDsl<SqliteConnection> for BatchQuery<'_> {}

impl QueryFragment<Sqlite> for BatchQuery<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        if !self.cacheable {
            pass.unsafe_to_cache_prepared();
        }
        let mut quoted = false;
        let mut start = 0;
        let mut binding_index = 0;
        let mut chars = self.sql.char_indices().peekable();
        while let Some((index, ch)) = chars.next() {
            match ch {
                '\'' if quoted && chars.peek().is_some_and(|(_, next)| *next == '\'') => {
                    chars.next();
                }
                '\'' => quoted = !quoted,
                '?' if !quoted => {
                    pass.push_sql(&self.sql[start..index]);
                    self.bindings.push_bind(binding_index, &mut pass)?;
                    binding_index += 1;
                    start = index + ch.len_utf8();
                }
                _ => {}
            }
        }
        if binding_index != self.bindings.len() {
            return Err(query_builder_error(
                "more batch bindings than SQL placeholders",
            ));
        }
        pass.push_sql(&self.sql[start..]);
        Ok(())
    }
}

fn query_builder_error(message: &str) -> diesel::result::Error {
    diesel::result::Error::QueryBuilderError(message.to_owned().into())
}

pub(super) struct Binding<ST, V> {
    value: V,
    sql_type: PhantomData<ST>,
}

pub(super) trait Bindings {
    fn walk<'b>(
        &'b self,
        pass: AstPass<'_, 'b, Sqlite>,
        segments: &mut Split<'_, char>,
    ) -> diesel::QueryResult<()>;
}

impl Bindings for () {
    fn walk<'b>(
        &'b self,
        _: AstPass<'_, 'b, Sqlite>,
        _: &mut Split<'_, char>,
    ) -> diesel::QueryResult<()> {
        Ok(())
    }
}

impl<B, ST, V> Bindings for (B, Binding<ST, V>)
where
    B: Bindings,
    Sqlite: HasSqlType<ST>,
    V: ToSql<ST, Sqlite>,
{
    fn walk<'b>(
        &'b self,
        mut pass: AstPass<'_, 'b, Sqlite>,
        segments: &mut Split<'_, char>,
    ) -> diesel::QueryResult<()> {
        self.0.walk(pass.reborrow(), segments)?;
        let sql = segments.next().ok_or_else(|| {
            diesel::result::Error::QueryBuilderError("more bindings than SQL placeholders".into())
        })?;
        pass.push_sql(sql);
        pass.push_bind_param::<ST, _>(&self.1.value)?;
        Ok(())
    }
}

impl<B> QueryId for CachedSql<B> {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<B> Query for CachedSql<B> {
    type SqlType = Untyped;
}

impl<B> RunQueryDsl<SqliteConnection> for CachedSql<B> {}

impl<B: Bindings> QueryFragment<Sqlite> for CachedSql<B> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        let mut segments = self.sql.split('?');
        self.bindings.walk(pass.reborrow(), &mut segments)?;
        let tail = segments.next().ok_or_else(|| {
            diesel::result::Error::QueryBuilderError("more bindings than SQL placeholders".into())
        })?;
        pass.push_sql(tail);
        if segments.next().is_some() {
            return Err(diesel::result::Error::QueryBuilderError(
                "fewer bindings than SQL placeholders".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use diesel::{
        Connection, RunQueryDsl, SqliteConnection,
        connection::{InstrumentationEvent, SimpleConnection},
        sql_types::{BigInt, Binary, Bool, Nullable, Text},
    };

    use super::{
        BorrowedBinding, InsertBatch, InsertPhase, OwnedBinding, cached_generated_sql,
        cached_generated_sql_borrowed, cached_sql,
    };

    #[derive(diesel::QueryableByName)]
    struct Row {
        #[diesel(sql_type = BigInt)]
        id: i64,
        #[diesel(sql_type = Nullable<Text>)]
        name: Option<String>,
    }

    #[derive(diesel::QueryableByName)]
    struct BatchCounts {
        #[diesel(sql_type = BigInt)]
        child_count: i64,
        #[diesel(sql_type = BigInt)]
        tail_count: i64,
    }

    #[derive(diesel::QueryableByName)]
    struct BorrowedRow {
        #[diesel(sql_type = Text)]
        algorithm: String,
        #[diesel(sql_type = diesel::sql_types::Binary)]
        payload: Vec<u8>,
        #[diesel(sql_type = Text)]
        note: String,
    }

    #[derive(diesel::QueryableByName)]
    struct AllBindingsRow {
        #[diesel(sql_type = BigInt)]
        id: i64,
        #[diesel(sql_type = Nullable<Text>)]
        name: Option<String>,
        #[diesel(sql_type = Nullable<BigInt>)]
        optional_id: Option<i64>,
        #[diesel(sql_type = Bool)]
        enabled: bool,
        #[diesel(sql_type = Nullable<Bool>)]
        optional_enabled: Option<bool>,
        #[diesel(sql_type = Binary)]
        payload: Vec<u8>,
        #[diesel(sql_type = Nullable<Binary>)]
        optional_payload: Option<Vec<u8>>,
    }

    #[test]
    fn generated_sql_accepts_borrowed_text_and_binary_bindings()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(
            "CREATE TABLE items(id INTEGER, algorithm TEXT, payload BLOB, note TEXT)",
        )?;
        let algorithm = String::from("sha1");
        let digest = [0x5a; 20];

        cached_generated_sql_borrowed(
            "INSERT INTO items(id,algorithm,payload,note) VALUES (?,?,?,'literal_?')".to_owned(),
            vec![
                BorrowedBinding::BigInt(7),
                BorrowedBinding::Text(&algorithm),
                BorrowedBinding::Binary(&digest),
            ],
        )
        .execute(&mut conn)?;

        let row = cached_sql("SELECT algorithm,payload,note FROM items WHERE id=7")
            .get_result::<BorrowedRow>(&mut conn)?;
        assert_eq!(row.algorithm, algorithm);
        assert_eq!(row.payload, digest);
        assert_eq!(row.note, "literal_?");
        Ok(())
    }

    #[test]
    fn fixed_sql_caches_by_statement_not_values_or_bind_types()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT)")?;
        let prepared = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&prepared);
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if matches!(event, InstrumentationEvent::CacheQuery { .. }) {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });
        cached_sql("INSERT INTO items(id,name) VALUES (?,?)")
            .bind::<BigInt, _>(1_i64)
            .bind::<Nullable<Text>, _>(Some("first"))
            .execute(&mut conn)?;
        cached_sql("INSERT INTO items(id,name) VALUES (?,?)")
            .bind::<BigInt, _>(2_i64)
            .bind::<Nullable<Text>, _>(None::<String>)
            .execute(&mut conn)?;
        let rows = cached_sql("SELECT id,name FROM items ORDER BY id").load::<Row>(&mut conn)?;
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].id, rows[0].name.as_deref()), (1, Some("first")));
        assert_eq!((rows[1].id, rows[1].name.as_deref()), (2, None));
        assert_eq!(prepared.load(Ordering::Relaxed), 2);
        Ok(())
    }

    #[test]
    fn generated_sql_reuses_owned_text_across_values_and_nulls()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT)")?;
        let cache_admissions = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&cache_admissions);
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::CacheQuery { sql, .. } = event
                && sql.starts_with("INSERT INTO items")
            {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        cached_generated_sql(
            "INSERT INTO items(id,name) VALUES (?,?)".into(),
            vec![
                OwnedBinding::BigInt(1),
                OwnedBinding::NullableText(Some("first".into())),
            ],
        )
        .execute(&mut conn)?;
        cached_generated_sql(
            "INSERT INTO items(id,name) VALUES (?,?)".into(),
            vec![OwnedBinding::BigInt(2), OwnedBinding::NullableText(None)],
        )
        .execute(&mut conn)?;

        let rows = cached_sql("SELECT id,name FROM items ORDER BY id").load::<Row>(&mut conn)?;
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].id, rows[0].name.as_deref()), (1, Some("first")));
        assert_eq!((rows[1].id, rows[1].name.as_deref()), (2, None));
        assert_eq!(cache_admissions.load(Ordering::Relaxed), 1);
        Ok(())
    }

    #[test]
    fn generated_sql_cache_isolated_by_text_for_equal_bind_types()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        let cache_admissions = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&cache_admissions);
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::CacheQuery { sql, .. } = event
                && sql.starts_with("SELECT")
            {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        let first = cached_generated_sql(
            "SELECT ? AS id, NULL AS name".into(),
            vec![OwnedBinding::BigInt(7)],
        )
        .get_result::<Row>(&mut conn)?;
        let second = cached_generated_sql(
            "SELECT ? + 1 AS id, NULL AS name".into(),
            vec![OwnedBinding::BigInt(7)],
        )
        .get_result::<Row>(&mut conn)?;

        assert_eq!((first.id, second.id), (7, 8));
        assert_eq!(cache_admissions.load(Ordering::Relaxed), 2);
        Ok(())
    }

    #[test]
    fn bindings_must_match_placeholders() -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        assert!(matches!(
            cached_sql("SELECT ?").load::<Row>(&mut conn),
            Err(diesel::result::Error::QueryBuilderError(_))
        ));
        assert!(matches!(
            cached_sql("SELECT 1")
                .bind::<BigInt, _>(1_i64)
                .load::<Row>(&mut conn),
            Err(diesel::result::Error::QueryBuilderError(_))
        ));
        Ok(())
    }

    #[test]
    fn different_sql_with_equal_bind_types_does_not_reuse_the_wrong_statement()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        let first = cached_sql("SELECT ? AS id, NULL AS name")
            .bind::<BigInt, _>(7_i64)
            .get_result::<Row>(&mut conn)?;
        let second = cached_sql("SELECT ? + 1 AS id, NULL AS name")
            .bind::<BigInt, _>(7_i64)
            .get_result::<Row>(&mut conn)?;
        assert_eq!((first.id, second.id), (7, 8));
        Ok(())
    }

    #[test]
    fn batches_owned_rows_with_nullable_and_binary_values() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(
            "CREATE TABLE items(
                id INTEGER PRIMARY KEY,
                name TEXT,
                optional_id INTEGER,
                enabled BOOLEAN NOT NULL,
                optional_enabled BOOLEAN,
                payload BLOB NOT NULL,
                optional_payload BLOB
            )",
        )?;
        let mut batch = InsertBatch::new();
        cached_sql("INSERT INTO items(id,name,optional_id,enabled,optional_enabled,payload,optional_payload) VALUES (?,?,?,?,?,?,?)")
            .bind::<BigInt, _>(1_i64)
            .bind::<Nullable<Text>, _>(Some("first"))
            .bind::<Nullable<BigInt>, _>(Some(17_i64))
            .bind::<Bool, _>(true)
            .bind::<Nullable<Bool>, _>(Some(false))
            .bind::<diesel::sql_types::Binary, _>(vec![0, 1, 255])
            .bind::<Nullable<Binary>, _>(Some(vec![9, 8]))
            .enqueue(&mut batch, &mut conn, InsertPhase::Parents)?;
        cached_sql("INSERT INTO items(id,name,optional_id,enabled,optional_enabled,payload,optional_payload) VALUES (?,?,?,?,?,?,?)")
            .bind::<BigInt, _>(2_i64)
            .bind::<Nullable<Text>, _>(None::<String>)
            .bind::<Nullable<BigInt>, _>(None)
            .bind::<Bool, _>(false)
            .bind::<Nullable<Bool>, _>(None)
            .bind::<diesel::sql_types::Binary, _>(vec![42])
            .bind::<Nullable<Binary>, _>(None)
            .enqueue(&mut batch, &mut conn, InsertPhase::Parents)?;
        batch.flush(&mut conn)?;

        let rows = cached_sql(
            "SELECT id,name,optional_id,enabled,optional_enabled,payload,optional_payload \
             FROM items ORDER BY id",
        )
        .load::<AllBindingsRow>(&mut conn)?;
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows[0].id,
                rows[0].name.as_deref(),
                rows[0].optional_id,
                rows[0].enabled,
                rows[0].optional_enabled,
            ),
            (1, Some("first"), Some(17), true, Some(false))
        );
        assert_eq!(rows[0].payload, [0, 1, 255]);
        assert_eq!(rows[0].optional_payload.as_deref(), Some(&[9, 8][..]));
        assert_eq!(
            (
                rows[1].id,
                rows[1].name.as_deref(),
                rows[1].optional_id,
                rows[1].enabled,
                rows[1].optional_enabled,
            ),
            (2, None, None, false, None)
        );
        assert_eq!(rows[1].payload, [42]);
        assert_eq!(rows[1].optional_payload, None);
        Ok(())
    }

    #[test]
    fn batch_groups_sql_shapes_in_first_seen_dependency_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(
            "PRAGMA foreign_keys=ON;
            CREATE TABLE parents(id INTEGER PRIMARY KEY);
            CREATE TABLE children(parent_id INTEGER REFERENCES parents(id), value TEXT);",
        )?;
        let mut batch = InsertBatch::new();
        cached_sql("INSERT INTO children(parent_id,value) VALUES (?,?)")
            .bind::<BigInt, _>(1_i64)
            .bind::<Text, _>("child")
            .enqueue(&mut batch, &mut conn, InsertPhase::Children)?;
        cached_sql("INSERT INTO parents(id) VALUES (?)")
            .bind::<BigInt, _>(1_i64)
            .enqueue(&mut batch, &mut conn, InsertPhase::Parents)?;

        batch.flush(&mut conn)?;
        let child = cached_sql(
            "SELECT parent_id AS id,name FROM (SELECT parent_id,value AS name FROM children)",
        )
        .get_result::<Row>(&mut conn)?;
        assert_eq!((child.id, child.name.as_deref()), (1, Some("child")));
        Ok(())
    }

    #[test]
    fn byte_capacity_flush_relooks_up_group_after_phase_sort()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(
            "CREATE TABLE parents(id INTEGER, value TEXT);
             CREATE TABLE children(parent_id INTEGER, value TEXT);",
        )?;
        let mut batch = InsertBatch::new();
        let first_value = "c".repeat(7 * 1024 * 1024);
        let parent_value = "p".repeat(7 * 1024 * 1024);
        let tail_value = "t".repeat(3 * 1024 * 1024);

        cached_sql("INSERT INTO children(parent_id,value) VALUES (?,?)")
            .bind::<BigInt, _>(10_i64)
            .bind::<Text, _>(first_value)
            .enqueue(&mut batch, &mut conn, InsertPhase::Children)?;
        cached_sql("INSERT INTO parents(id,value) VALUES (?,?)")
            .bind::<BigInt, _>(10_i64)
            .bind::<Text, _>(parent_value)
            .enqueue(&mut batch, &mut conn, InsertPhase::Parents)?;

        // The pending row crosses the global byte bound. Flushing sorts the
        // existing groups, changing the child group's index before this row
        // is appended.
        cached_sql("INSERT INTO children(parent_id,value) VALUES (?,?)")
            .bind::<BigInt, _>(20_i64)
            .bind::<Text, _>(tail_value)
            .enqueue(&mut batch, &mut conn, InsertPhase::Children)?;
        batch.flush(&mut conn)?;

        let counts = cached_sql(
            "SELECT (SELECT count(*) FROM children) AS child_count, \
                    (SELECT count(*) FROM children WHERE parent_id=20) AS tail_count",
        )
        .get_result::<BatchCounts>(&mut conn)?;
        assert_eq!((counts.child_count, counts.tail_count), (2, 1));
        Ok(())
    }

    #[test]
    fn variable_tail_sizes_do_not_grow_prepared_cache_and_preserve_rows()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("CREATE TABLE items(id INTEGER PRIMARY KEY, value TEXT)")?;
        let cache_hits = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&cache_hits);
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::CacheQuery { sql, .. } = event
                && sql.starts_with("INSERT INTO items")
            {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        let mut next_id = 0_i64;
        for _ in 0..2 {
            for rows in 1..=111 {
                let mut batch = InsertBatch::new();
                for _ in 0..rows {
                    let id = next_id;
                    next_id += 1;
                    cached_sql("INSERT INTO items(id,value) VALUES (?,?)")
                        .bind::<BigInt, _>(id)
                        .bind::<diesel::sql_types::Text, _>(format!("item-{id}"))
                        .enqueue(&mut batch, &mut conn, InsertPhase::Parents)?;
                }
                batch.flush(&mut conn)?;
            }
        }

        let count = cached_sql("SELECT count(*) AS id, NULL AS name FROM items")
            .get_result::<Row>(&mut conn)?;
        assert_eq!(count.id, next_id);
        let tail = cached_sql("SELECT id,value AS name FROM items WHERE id=?")
            .bind::<BigInt, _>(next_id - 1)
            .get_result::<Row>(&mut conn)?;
        let expected_tail = format!("item-{}", next_id - 1);
        assert_eq!(
            (tail.id, tail.name.as_deref()),
            (next_id - 1, Some(expected_tail.as_str()))
        );
        // Of sizes 1..=111, only seven powers of two are admitted. Each is
        // reused once on the second pass; all other tails bypass the cache.
        assert_eq!(cache_hits.load(Ordering::Relaxed), 7);
        Ok(())
    }

    #[test]
    fn failed_batch_can_be_rolled_back_as_a_unit() -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(
            "PRAGMA foreign_keys=ON;
            CREATE TABLE parents(id INTEGER PRIMARY KEY);
            CREATE TABLE children(parent_id INTEGER REFERENCES parents(id), value TEXT);",
        )?;

        let result = conn.transaction::<(), diesel::result::Error, _>(|conn| {
            let mut batch = InsertBatch::new();
            cached_sql("INSERT INTO parents(id) VALUES (?)")
                .bind::<BigInt, _>(1_i64)
                .enqueue(&mut batch, conn, InsertPhase::Parents)
                .map_err(|error| {
                    diesel::result::Error::QueryBuilderError(error.to_string().into())
                })?;
            cached_sql("INSERT INTO children(parent_id,value) VALUES (?,?)")
                .bind::<BigInt, _>(99_i64)
                .bind::<Text, _>("orphan")
                .enqueue(&mut batch, conn, InsertPhase::Children)
                .map_err(|error| {
                    diesel::result::Error::QueryBuilderError(error.to_string().into())
                })?;
            batch.flush(conn)
        });
        assert!(result.is_err());
        let count = cached_sql("SELECT count(*) AS id, NULL AS name FROM parents")
            .get_result::<Row>(&mut conn)?;
        assert_eq!(count.id, 0);
        Ok(())
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "opening an in-memory database is required test setup"
    )]
    fn batch_rejects_unsupported_insert_shapes() {
        let mut conn = SqliteConnection::establish(":memory:").expect("open sqlite");
        let mut batch = InsertBatch::new();
        assert!(
            cached_sql("INSERT INTO items(id) VALUES (?) RETURNING id")
                .bind::<BigInt, _>(1_i64)
                .enqueue(&mut batch, &mut conn, InsertPhase::Parents)
                .is_err()
        );
        assert!(
            cached_sql("INSERT INTO items(id) VALUES (?),(?)")
                .bind::<BigInt, _>(1_i64)
                .enqueue(&mut batch, &mut conn, InsertPhase::Parents)
                .is_err()
        );
    }

    #[test]
    fn batch_preserves_quoted_constants_containing_placeholders()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("CREATE TABLE literals(id INTEGER, declared TEXT)")?;
        let mut batch = InsertBatch::new();
        cached_sql("INSERT INTO literals(id,declared) VALUES (?, 'source_?''declared')")
            .bind::<BigInt, _>(7_i64)
            .enqueue(&mut batch, &mut conn, InsertPhase::Parents)?;
        batch.flush(&mut conn)?;
        let row =
            cached_sql("SELECT id,declared AS name FROM literals").get_result::<Row>(&mut conn)?;
        assert_eq!(
            (row.id, row.name.as_deref()),
            (7, Some("source_?'declared"))
        );
        Ok(())
    }
}
