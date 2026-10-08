//! Constructed publication fixtures; no claim of authentic parser capture/EOF.

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use super::catalog_ids::EditionId;

#[derive(Debug, QueryableByName)]
struct FixtureClosureProblem {
    #[diesel(sql_type = Text)]
    problem: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    owner_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    edition_id: Option<i64>,
}

/// Install one minimal native MAME publication with all canonical guards active.
/// The selected edition must already exist and its source must cover 64 bytes.
pub(crate) fn publish_minimal_mame(
    connection: &mut SqliteConnection,
    edition: EditionId,
    published_at: &str,
) -> crate::Result<()> {
    publish_minimal_mame_fixture(connection, edition, published_at, false)
}

/// Install a minimal native MAME publication with one unlinked ROM carrying
/// an invalid, unknown-scope CRC declaration. All canonical guards remain active.
/// The selected edition must already exist and its source must cover 64 bytes.
pub(crate) fn publish_minimal_mame_with_rom(
    connection: &mut SqliteConnection,
    edition: EditionId,
    published_at: &str,
) -> crate::Result<()> {
    publish_minimal_mame_fixture(connection, edition, published_at, true)
}

fn publish_minimal_mame_fixture(
    connection: &mut SqliteConnection,
    edition: EditionId,
    published_at: &str,
    include_rom: bool,
) -> crate::Result<()> {
    let edition = edition.as_i64();
    let group = edition.checked_mul(10).ok_or_else(overflow)?;
    let set = group.checked_add(1).ok_or_else(overflow)?;
    let text = group.checked_add(2).ok_or_else(overflow)?;
    let mut fixture_sql = format!(
        "INSERT INTO mame_documents
         (edition_id,debug,debug_specified,mameconfig,source_line,source_column,
          extent_view,extent_start,extent_end,location_view,start_line,start_column,
          end_line,end_column,column_convention)
         VALUES({edition},0,0,'',1,1,'retained_original_bytes',0,64,
                'transport_decoded_xml_text',1,1,2,1,'one_based_unicode_scalar');
         INSERT INTO mame_document_facts_attribute_positions VALUES({edition},'mameconfig',0,0,1,7);
         INSERT INTO catalog_set_groups VALUES({group},{edition},'root');
         INSERT INTO catalog_source_elements VALUES({set},{edition},'mame_machine');
         INSERT INTO catalog_sets VALUES({set},{group},'machine',0,1,1);
         INSERT INTO mame_machines VALUES({set},NULL,0,0,0,0,0,0,1,0);
         INSERT INTO mame_machines_attribute_positions VALUES({set},'name',0,NULL,0,1,1);
         INSERT INTO catalog_source_elements VALUES({text},{edition},'mame_machine_text');
         INSERT INTO mame_machine_text_elements VALUES({text},{set},'description','',0,1,1);"
    );
    if include_rom {
        let media = group.checked_add(3).ok_or_else(overflow)?;
        let hash = group.checked_add(4).ok_or_else(overflow)?;
        let empty_set = group.checked_add(5).ok_or_else(overflow)?;
        let empty_text = group.checked_add(6).ok_or_else(overflow)?;
        fixture_sql.push_str(&format!(
            "INSERT INTO catalog_source_elements VALUES({media},{edition},'mame_rom');
             INSERT INTO catalog_media_entries VALUES({media},NULL);
             INSERT INTO mame_roms
             (media_entry_id,machine_id,name,size_text,bios,region,offset,status,status_specified,
              optional,optional_specified,source_order,source_line,source_column)
             VALUES({media},{set},'fixture.rom','4',NULL,NULL,NULL,'good',0,0,0,1,1,1);
             INSERT INTO catalog_entry_hashes
             (reported_hash_id,media_entry_id,source_hash_field,field_occurrence,presence,
              hash_scope,hash_id,reported_text)
             VALUES({hash},{media},'crc',0,'invalid','unknown',NULL,'not-hex');
             INSERT INTO invalid_catalog_entry_hashes VALUES({hash},'constructed_invalid_hex');
             INSERT INTO mame_rom_claims_attribute_positions
             (media_entry_id,field_kind,field_occurrence,reported_hash_id,relationship_id,
              source_order,source_line,source_column)
             VALUES({media},'name',0,NULL,NULL,0,1,1),
                   ({media},'size',0,NULL,NULL,1,1,15),
                   ({media},'crc',0,{hash},NULL,2,1,23);
             INSERT INTO catalog_source_elements VALUES({empty_set},{edition},'mame_machine');
             INSERT INTO catalog_sets VALUES({empty_set},{group},'empty-machine',1,2,1);
             INSERT INTO mame_machines VALUES({empty_set},NULL,0,0,0,0,0,0,1,0);
             INSERT INTO mame_machines_attribute_positions VALUES({empty_set},'name',0,NULL,0,2,1);
             INSERT INTO catalog_source_elements VALUES({empty_text},{edition},'mame_machine_text');
             INSERT INTO mame_machine_text_elements VALUES({empty_text},{empty_set},'description','',0,2,1);"
        ));
    }
    connection.batch_execute(&fixture_sql)?;
    let counters = include_str!("../../docs/schema-candidate/mame-counts.tsv")
        .lines()
        .skip(1)
        .map(|line| {
            line.split('\t').nth(1).ok_or_else(|| {
                crate::Error::DatabaseSchema("invalid test counter inventory".to_owned())
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    // Literal source events of this fixture, never counts inferred from SQL.
    let values = counters
        .iter()
        .map(|counter| match *counter {
            "machine_count" | "machine_text_element_count" | "machine_attribute_position_count"
                if include_rom =>
            {
                "2"
            }
            "machine_count"
            | "machine_text_element_count"
            | "document_attribute_position_count"
            | "machine_attribute_position_count" => "1",
            "rom_count" if include_rom => "1",
            "rom_attribute_position_count" if include_rom => "3",
            _ => "0",
        })
        .collect::<Vec<_>>();
    connection.batch_execute(&format!(
        "INSERT INTO mame_source_count_seals(edition_id,{}) VALUES({edition},{});",
        counters.join(","),
        values.join(",")
    ))?;
    let publication = sql_query(
        "INSERT INTO published_catalog_editions \
		 (edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id,published_at) \
		 SELECT edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id,? \
		 FROM catalog_editions WHERE edition_id=?",
    )
    .bind::<Text, _>(published_at)
    .bind::<BigInt, _>(edition)
    .execute(connection);
    if let Err(publication_error) = publication {
        // Inspect the canonical closure view only after the real publication guard
        // rejects this fixture; never weaken the guard or infer source counters.
        let problems = sql_query(
            "SELECT problem, owner_id, edition_id FROM candidate_integrity_problems \
			 WHERE edition_id=? ORDER BY problem, owner_id LIMIT 50",
        )
        .bind::<BigInt, _>(edition)
        .load::<FixtureClosureProblem>(connection)?;
        let findings = problems
            .into_iter()
            .map(|problem| {
                format!(
                    "{} (owner {:?}, edition {:?})",
                    problem.problem, problem.owner_id, problem.edition_id
                )
            })
            .collect::<Vec<_>>();
        return Err(crate::Error::DatabaseSchema(format!(
            "test MAME fixture publication failed: {publication_error}; canonical closure findings: {findings:?}"
        )));
    }
    Ok(())
}

fn overflow() -> crate::Error {
    crate::Error::DatabaseSchema("test fixture identifier overflow".to_owned())
}
