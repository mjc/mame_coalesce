//! Bulk interning for catalog digest values.

use diesel::{Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_types::BigInt};

use super::{
    cached_sql::{BorrowedBinding, cached_generated_sql_borrowed},
    catalog_ids::HashId,
};

const HASHES_PER_PAGE: usize = 128;

/// One of the digest algorithms persisted by the catalog schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// CRC-32, encoded as four binary bytes.
    Crc32,
    /// MD5, encoded as sixteen binary bytes.
    Md5,
    /// SHA-1, encoded as twenty binary bytes.
    Sha1,
    /// SHA-256, encoded as thirty-two binary bytes.
    Sha256,
}

impl HashAlgorithm {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Crc32 => "crc32",
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }

    pub(crate) const fn byte_length(self) -> usize {
        match self {
            Self::Crc32 => 4,
            Self::Md5 => 16,
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }

    /// Decode an algorithm persisted by the canonical catalog schema.
    pub(crate) fn from_database(value: &str) -> crate::Result<Self> {
        match value {
            "crc32" => Ok(Self::Crc32),
            "md5" => Ok(Self::Md5),
            "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            _ => Err(crate::Error::DatabaseSchema(format!(
                "invalid persisted hash_values.algorithm: {value}"
            ))),
        }
    }
}

/// A parsed binary digest to intern. `bytes` is the digest value, not its hex spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HashInput<'a> {
    /// Digest algorithm.
    pub algorithm: HashAlgorithm,
    /// Binary digest bytes.
    pub bytes: &'a [u8],
}

#[derive(QueryableByName)]
struct ResolvedHashRow {
    #[diesel(sql_type = BigInt)]
    input_order: i64,
    #[diesel(sql_type = BigInt)]
    hash_id: i64,
}

/// Intern digest values in bulk and return one ID per input, in input order.
///
/// Repeated `(algorithm, bytes)` pairs share an ID, including repetitions
/// within this call. Every digest is validated before any SQL is executed.
pub fn intern(
    connection: &mut SqliteConnection,
    inputs: &[HashInput<'_>],
) -> crate::Result<Vec<HashId>> {
    for input in inputs {
        let expected = input.algorithm.byte_length();
        if input.bytes.len() != expected {
            return Err(crate::Error::InvalidHash(format!(
                "{} digest must contain {expected} bytes, got {}",
                input.algorithm.as_str(),
                input.bytes.len()
            )));
        }
    }

    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    connection.transaction(|connection| intern_validated(connection, inputs))
}

fn intern_validated(
    connection: &mut SqliteConnection,
    inputs: &[HashInput<'_>],
) -> crate::Result<Vec<HashId>> {
    let mut resolved = Vec::with_capacity(inputs.len());
    for page in inputs.chunks(HASHES_PER_PAGE) {
        let arity = page.len().next_power_of_two();
        let mut values = Vec::with_capacity(arity * 3);
        for row in 0..arity {
            let input_order = row.min(page.len() - 1);
            let input = page[input_order];
            values.push(BorrowedBinding::BigInt(
                i64::try_from(input_order).map_err(|_| {
                    crate::Error::DatabaseSchema("too many digests in one page".to_owned())
                })?,
            ));
            values.push(BorrowedBinding::Text(input.algorithm.as_str()));
            values.push(BorrowedBinding::Binary(input.bytes));
        }

        let rows = std::iter::repeat_n("(?,?,?)", arity)
            .collect::<Vec<_>>()
            .join(",");
        let values_cte =
            format!("WITH requested(input_order, algorithm, bytes) AS (VALUES {rows}) ");
        let mut insert_bindings = Vec::with_capacity(arity * 2);
        for row in 0..arity {
            let input = page[row.min(page.len() - 1)];
            insert_bindings.push(BorrowedBinding::Text(input.algorithm.as_str()));
            insert_bindings.push(BorrowedBinding::Binary(input.bytes));
        }
        let insert_rows = std::iter::repeat_n("(?,?)", arity)
            .collect::<Vec<_>>()
            .join(",");
        cached_generated_sql_borrowed(
            format!(
                "WITH requested(algorithm, bytes) AS (VALUES {insert_rows}) \
                 INSERT INTO hash_values(algorithm, bytes) \
                 SELECT DISTINCT requested.algorithm, requested.bytes FROM requested \
                 WHERE NOT EXISTS (SELECT 1 FROM hash_values AS existing \
                                   WHERE existing.algorithm=requested.algorithm \
                                     AND existing.bytes=requested.bytes)"
            ),
            insert_bindings,
        )
        .execute(connection)?;

        let resolved_rows = cached_generated_sql_borrowed(
            format!(
                "{values_cte} SELECT DISTINCT requested.input_order, value.hash_id \
                 FROM requested JOIN hash_values AS value \
                   ON value.algorithm=requested.algorithm AND value.bytes=requested.bytes \
                 ORDER BY requested.input_order"
            ),
            values,
        )
        .load::<ResolvedHashRow>(connection)?;

        if resolved_rows.len() != page.len() {
            return Err(crate::Error::DatabaseSchema(
                "bulk hash intern did not resolve every input digest".to_owned(),
            ));
        }
        for (expected_order, row) in resolved_rows.into_iter().enumerate() {
            if usize::try_from(row.input_order).ok() != Some(expected_order) {
                return Err(crate::Error::DatabaseSchema(
                    "bulk hash intern returned noncanonical input ordering".to_owned(),
                ));
            }
            resolved.push(HashId::try_from(row.hash_id)?);
        }
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests;
