//! Cached statements for the finite, repository-owned native import SQL.
//!
//! SQL is keyed by its text, never by the Rust type of its bind values. Only
//! audited static statements with anonymous `?` placeholders are accepted;
//! placeholders inside SQL string literals are deliberately unsupported.

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

pub(super) const fn cached_sql(sql: &'static str) -> CachedSql {
    CachedSql { sql, bindings: () }
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
        sql_types::{BigInt, Nullable, Text},
    };

    use super::cached_sql;

    #[derive(diesel::QueryableByName)]
    struct Row {
        #[diesel(sql_type = BigInt)]
        id: i64,
        #[diesel(sql_type = Nullable<Text>)]
        name: Option<String>,
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
}
