use diesel::{
    Connection, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Text},
};

use super::{HASHES_PER_PAGE, HashAlgorithm, HashInput, intern};
use crate::storage::catalog_ids::HashId;

fn connection() -> crate::Result<SqliteConnection> {
    let mut connection = SqliteConnection::establish(":memory:")
        .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
    crate::storage::db::initialize_database(&mut connection)?;
    Ok(connection)
}

#[derive(diesel::QueryableByName)]
struct StoredHash {
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Binary)]
    bytes: Vec<u8>,
}

#[derive(diesel::QueryableByName)]
struct HashCount {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn stored_hash(connection: &mut SqliteConnection, id: HashId) -> crate::Result<StoredHash> {
    Ok(
        sql_query("SELECT algorithm, bytes FROM hash_values WHERE hash_id = ?")
            .bind::<BigInt, _>(id.as_i64())
            .get_result(connection)?,
    )
}

fn hash_count(connection: &mut SqliteConnection) -> crate::Result<i64> {
    Ok(sql_query("SELECT COUNT(*) AS count FROM hash_values")
        .get_result::<HashCount>(connection)?
        .count)
}

#[test]
fn interns_all_algorithms_and_returns_input_order_with_duplicates() -> crate::Result<()> {
    let mut connection = connection()?;
    let crc32 = [0x11; 4];
    let md5 = [0x22; 16];
    let sha1 = [0x33; 20];
    let sha256 = [0x44; 32];
    let inputs = [
        HashInput {
            algorithm: HashAlgorithm::Crc32,
            bytes: &crc32,
        },
        HashInput {
            algorithm: HashAlgorithm::Sha1,
            bytes: &sha1,
        },
        HashInput {
            algorithm: HashAlgorithm::Md5,
            bytes: &md5,
        },
        HashInput {
            algorithm: HashAlgorithm::Sha256,
            bytes: &sha256,
        },
        HashInput {
            algorithm: HashAlgorithm::Sha1,
            bytes: &sha1,
        },
    ];

    let ids = intern(&mut connection, &inputs)?;

    assert_eq!(ids.len(), inputs.len());
    assert_eq!(ids[1], ids[4]);
    for (id, algorithm, bytes) in [
        (ids[0], "crc32", crc32.as_slice()),
        (ids[1], "sha1", sha1.as_slice()),
        (ids[2], "md5", md5.as_slice()),
        (ids[3], "sha256", sha256.as_slice()),
        (ids[4], "sha1", sha1.as_slice()),
    ] {
        let stored = stored_hash(&mut connection, id)?;
        assert_eq!(stored.algorithm, algorithm);
        assert_eq!(stored.bytes, bytes);
    }
    assert_eq!(hash_count(&mut connection)?, 4);
    Ok(())
}

#[test]
fn reuses_interned_hash_ids_across_calls() -> crate::Result<()> {
    let mut connection = connection()?;
    let bytes = [0x55; 20];
    let input = [HashInput {
        algorithm: HashAlgorithm::Sha1,
        bytes: &bytes,
    }];

    let first = intern(&mut connection, &input)?;
    let second = intern(&mut connection, &input)?;

    assert_eq!(first, second);
    assert_eq!(hash_count(&mut connection)?, 1);
    Ok(())
}

#[test]
fn interns_borrowed_digest_slices_across_the_existing_page_boundary() -> crate::Result<()> {
    let mut connection = connection()?;
    let digests = (0_u32..u32::try_from(HASHES_PER_PAGE).expect("page size fits"))
        .map(u32::to_be_bytes)
        .collect::<Vec<_>>();
    let mut inputs = digests
        .iter()
        .map(|bytes| HashInput {
            algorithm: HashAlgorithm::Crc32,
            bytes,
        })
        .collect::<Vec<_>>();
    inputs.push(HashInput {
        algorithm: HashAlgorithm::Crc32,
        bytes: &digests[0],
    });

    let ids = intern(&mut connection, &inputs)?;

    assert_eq!(ids.len(), HASHES_PER_PAGE + 1);
    assert_eq!(ids[0], ids[HASHES_PER_PAGE]);
    assert_eq!(
        hash_count(&mut connection)?,
        i64::try_from(HASHES_PER_PAGE).expect("page size fits SQLite")
    );
    Ok(())
}

#[test]
fn rejects_invalid_digest_lengths_without_partial_inserts() -> crate::Result<()> {
    let mut connection = connection()?;
    let valid = [0x66; 16];
    let invalid = [0x77; 3];
    let inputs = [
        HashInput {
            algorithm: HashAlgorithm::Md5,
            bytes: &valid,
        },
        HashInput {
            algorithm: HashAlgorithm::Crc32,
            bytes: &invalid,
        },
    ];

    assert!(intern(&mut connection, &inputs).is_err());
    assert_eq!(hash_count(&mut connection)?, 0);
    Ok(())
}

#[test]
fn rolls_back_earlier_pages_when_a_later_page_fails() -> crate::Result<()> {
    let mut connection = connection()?;
    connection.batch_execute(
        "CREATE TRIGGER reject_sha256 BEFORE INSERT ON hash_values \
         WHEN NEW.algorithm = 'sha256' \
         BEGIN SELECT RAISE(ABORT, 'test rejection'); END;",
    )?;
    let bytes = (0_u32..)
        .take(HASHES_PER_PAGE)
        .map(u32::to_be_bytes)
        .collect::<Vec<_>>();
    let sha256 = [0x88; 32];
    let mut inputs = bytes
        .iter()
        .map(|bytes| HashInput {
            algorithm: HashAlgorithm::Crc32,
            bytes,
        })
        .collect::<Vec<_>>();
    inputs.push(HashInput {
        algorithm: HashAlgorithm::Sha256,
        bytes: &sha256,
    });

    assert!(intern(&mut connection, &inputs).is_err());
    assert_eq!(hash_count(&mut connection)?, 0);
    Ok(())
}
