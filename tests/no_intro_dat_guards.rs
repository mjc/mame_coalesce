#![allow(clippy::expect_used)]

use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CONTENT_UUID: &[u8] = &[7; 16];

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct SchemaSql {
    #[diesel(sql_type = Text)]
    sql: String,
}

#[derive(QueryableByName)]
struct QueryPlan {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[derive(Default)]
struct Counts {
    games: i64,
    roms: i64,
    categories: i64,
    identifiers: i64,
    releases: i64,
    header_fields: i64,
    clr_options: i64,
    romcenter_options: i64,
    clr_fields: i64,
    romcenter_fields: i64,
    game_fields: i64,
    rom_fields: i64,
}

struct Owner {
    key: String,
    group: i64,
    game: i64,
    counts: Counts,
}

#[derive(Clone, Copy, Debug)]
enum Child {
    Category,
    Identifier,
    Rom,
    Release,
}

#[derive(Clone, Copy, Debug)]
enum Hash {
    Crc(u8),
    Md5(u8),
    Sha1(u8),
    Sha256(u8),
}

impl Hash {
    fn parts(self) -> (i64, &'static str, Vec<u8>) {
        match self {
            Self::Crc(value) => (2, "crc32", vec![value; 4]),
            Self::Md5(value) => (3, "md5", vec![value; 16]),
            Self::Sha1(value) => (4, "sha1", vec![value; 20]),
            Self::Sha256(value) => (5, "sha256", vec![value; 32]),
        }
    }
}

struct Fixture {
    connection: SqliteConnection,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let mut connection = SqliteConnection::establish(":memory:")?;
        connection.batch_execute(concat!(
            include_str!("../src/storage/db/coverage.sql"),
            "\n",
            include_str!("../src/storage/db/schema.sql"),
            "\n",
            include_str!("../src/storage/db/logiqx.sql"),
            "\n",
            include_str!("../src/storage/db/cmp.sql"),
            "\n",
            include_str!("../src/storage/db/catalog_registry.sql"),
            "\n",
            include_str!("../src/storage/db/no_intro_dat.sql"),
            "\n",
            include_str!("../src/storage/db/no_intro_dat_guards.sql"),
        ))?;
        connection.batch_execute(
            "PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=OFF;
             INSERT INTO publishing_sources(source_key,display_name) VALUES('source','Source');
             INSERT INTO documents(document_key) VALUES('document');
             INSERT INTO parser_interpretations(interpretation_key,format) VALUES('mode','no-intro-dat-v4-compatible');
             INSERT INTO catalog_coverage(coverage_id,kind) VALUES(1,'complete');
             INSERT INTO catalog_contents(content_uuid) VALUES(x'07070707070707070707070707070707');",
        )?;
        Ok(Self { connection })
    }

    fn owner(&mut self, key: &str, game_source_order: i64) -> TestResult<Owner> {
        self.owner_with_header_order(key, 0, game_source_order)
    }

    fn owner_with_header_order(
        &mut self,
        key: &str,
        header_source_order: i64,
        game_source_order: i64,
    ) -> TestResult<Owner> {
        sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) VALUES(?,'source',?)")
            .bind::<Text, _>(key)
            .bind::<Text, _>(key)
            .execute(&mut self.connection)?;
        sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) VALUES(?,?,'document','mode',1)")
            .bind::<Text, _>(key).bind::<Text, _>(key).execute(&mut self.connection)?;
        sql_query("INSERT INTO no_intro_dat_documents(snapshot_key,source_line,source_column) VALUES(?,1,1)")
            .bind::<Text, _>(key).execute(&mut self.connection)?;
        sql_query("INSERT INTO no_intro_dat_headers(snapshot_key,source_order,source_line,source_column,name,description) VALUES(?,?,1,1,'Catalog','Description')")
            .bind::<Text, _>(key).bind::<BigInt, _>(header_source_order).execute(&mut self.connection)?;
        sql_query("INSERT INTO no_intro_dat_header_field_positions(snapshot_key,field_kind,source_order,source_line,source_column) VALUES(?,1,0,2,1),(?,2,1,3,1)")
            .bind::<Text, _>(key).bind::<Text, _>(key).execute(&mut self.connection)?;
        let group = sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES(?,'root',0) RETURNING set_group_id AS value")
            .bind::<Text, _>(key).get_result::<Id>(&mut self.connection)?.value;
        let mut owner = Owner {
            key: key.into(),
            group,
            game: 0,
            counts: Counts {
                header_fields: 2,
                ..Counts::default()
            },
        };
        owner.game = self.game(&mut owner, game_source_order)?;
        Ok(owner)
    }

    fn game(&mut self, owner: &mut Owner, source_order: i64) -> TestResult<i64> {
        let game = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES(?,'no_intro_dat_game',?,'Game',50,1) RETURNING set_id AS value")
            .bind::<BigInt, _>(owner.group).bind::<BigInt, _>(owner.counts.games)
            .get_result::<Id>(&mut self.connection)?.value;
        sql_query("INSERT INTO no_intro_dat_games(set_id,source_order,description_text) VALUES(?,?,'Description')")
            .bind::<BigInt, _>(game).bind::<BigInt, _>(source_order).execute(&mut self.connection)?;
        sql_query("INSERT INTO no_intro_dat_game_field_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,50,2),(?,4,0,51,1)")
            .bind::<BigInt, _>(game).bind::<BigInt, _>(game).execute(&mut self.connection)?;
        owner.counts.games += 1;
        owner.counts.game_fields += 2;
        Ok(game)
    }

    fn child(&mut self, owner: &mut Owner, child: Child, source_order: i64) -> TestResult {
        match child {
            Child::Category => {
                sql_query("INSERT INTO no_intro_dat_categories(set_id,category_order,category,source_order,source_line,source_column) VALUES(?,?,'Category',?,60,1)")
                    .bind::<BigInt, _>(owner.game).bind::<BigInt, _>(owner.counts.categories)
                    .bind::<BigInt, _>(source_order).execute(&mut self.connection)?;
                owner.counts.categories += 1;
            }
            Child::Identifier => {
                sql_query("INSERT INTO no_intro_dat_identifiers(set_id,identifier_order,identifier,source_order,source_line,source_column) VALUES(?,?,'0001',?,61,1)")
                    .bind::<BigInt, _>(owner.game).bind::<BigInt, _>(owner.counts.identifiers)
                    .bind::<BigInt, _>(source_order).execute(&mut self.connection)?;
                owner.counts.identifiers += 1;
            }
            Child::Rom => {
                self.rom(owner, source_order, None, None, &[])?;
            }
            Child::Release => {
                self.release(owner, source_order, 0, 1)?;
            }
        }
        Ok(())
    }

    fn release(
        &mut self,
        owner: &mut Owner,
        source_order: i64,
        name_order: i64,
        region_order: i64,
    ) -> TestResult {
        sql_query("INSERT INTO no_intro_dat_releases(set_id,release_order,name,region,source_order,source_line,source_column,name_source_order,name_source_line,name_source_column,region_source_order,region_source_line,region_source_column) VALUES(?,?,'Release','USA',?,70,1,?,70,3,?,70,8)")
            .bind::<BigInt, _>(owner.game).bind::<BigInt, _>(owner.counts.releases)
            .bind::<BigInt, _>(source_order).bind::<BigInt, _>(name_order).bind::<BigInt, _>(region_order)
            .execute(&mut self.connection)?;
        owner.counts.releases += 1;
        Ok(())
    }

    fn rom(
        &mut self,
        owner: &mut Owner,
        source_order: i64,
        size: Option<i64>,
        uuid: Option<&[u8]>,
        hashes: &[Hash],
    ) -> TestResult<i64> {
        let occurrence = sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) VALUES(?,?,'no_intro_dat_rom',?) RETURNING occurrence_id AS value")
            .bind::<BigInt, _>(owner.game).bind::<BigInt, _>(owner.counts.roms)
            .bind::<Nullable<Binary>, _>(uuid).get_result::<Id>(&mut self.connection)?.value;
        sql_query("INSERT INTO no_intro_dat_rom_claims(occurrence_id,name,size_text,evidence_scope,source_order,source_line,source_column) VALUES(?,'file.bin',?,'whole_file',?,65,1)")
            .bind::<BigInt, _>(occurrence).bind::<Nullable<Text>, _>(size.map(|value| value.to_string()))
            .bind::<BigInt, _>(source_order).execute(&mut self.connection)?;
        self.rom_position(occurrence, 0, 0)?;
        owner.counts.rom_fields += 1;
        if size.is_some() {
            self.rom_position(occurrence, 1, 1)?;
            owner.counts.rom_fields += 1;
        }
        for hash in hashes {
            let (kind, algorithm, bytes) = hash.parts();
            let digest = self.digest(algorithm, &bytes)?;
            self.assertion(occurrence, digest, "whole_file", "source_declared")?;
            sql_query("INSERT INTO no_intro_dat_rom_digest_fields(occurrence_id,field_kind,digest_id,invalid_text) VALUES(?,?,?,NULL)")
                .bind::<BigInt, _>(occurrence).bind::<BigInt, _>(kind).bind::<BigInt, _>(digest)
                .execute(&mut self.connection)?;
            self.rom_position(occurrence, kind, kind)?;
            owner.counts.rom_fields += 1;
        }
        owner.counts.roms += 1;
        Ok(occurrence)
    }

    fn rom_position(&mut self, occurrence: i64, field_kind: i64, source_order: i64) -> TestResult {
        sql_query("INSERT INTO no_intro_dat_rom_field_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,?,?,65,?)")
            .bind::<BigInt, _>(occurrence).bind::<BigInt, _>(field_kind).bind::<BigInt, _>(source_order)
            .bind::<BigInt, _>(source_order + 2).execute(&mut self.connection)?;
        Ok(())
    }

    fn digest(&mut self, algorithm: &str, bytes: &[u8]) -> TestResult<i64> {
        sql_query("INSERT OR IGNORE INTO digest_values(algorithm,digest) VALUES(?,?)")
            .bind::<Text, _>(algorithm)
            .bind::<Binary, _>(bytes)
            .execute(&mut self.connection)?;
        Ok(
            sql_query(
                "SELECT digest_id AS value FROM digest_values WHERE algorithm=? AND digest=?",
            )
            .bind::<Text, _>(algorithm)
            .bind::<Binary, _>(bytes)
            .get_result::<Id>(&mut self.connection)?
            .value,
        )
    }

    fn assertion(
        &mut self,
        occurrence: i64,
        digest: i64,
        scope: &str,
        provenance: &str,
    ) -> TestResult {
        sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) VALUES(?,?,?,?)")
            .bind::<BigInt, _>(occurrence).bind::<BigInt, _>(digest)
            .bind::<Text, _>(scope).bind::<Text, _>(provenance).execute(&mut self.connection)?;
        Ok(())
    }

    fn seal(&mut self, owner: &Owner) -> TestResult {
        let counts = &owner.counts;
        sql_query("INSERT INTO no_intro_dat_parse_counts(snapshot_key,game_count,rom_count,category_count,identifier_count,release_count,clrmamepro_option_count,romcenter_option_count,header_field_count,clrmamepro_field_count,romcenter_field_count,game_field_count,rom_field_count) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind::<Text, _>(&owner.key).bind::<BigInt, _>(counts.games).bind::<BigInt, _>(counts.roms)
            .bind::<BigInt, _>(counts.categories).bind::<BigInt, _>(counts.identifiers).bind::<BigInt, _>(counts.releases)
            .bind::<BigInt, _>(counts.clr_options).bind::<BigInt, _>(counts.romcenter_options)
            .bind::<BigInt, _>(counts.header_fields).bind::<BigInt, _>(counts.clr_fields)
            .bind::<BigInt, _>(counts.romcenter_fields).bind::<BigInt, _>(counts.game_fields)
            .bind::<BigInt, _>(counts.rom_fields).execute(&mut self.connection)?;
        Ok(())
    }

    fn publish(&mut self, owner: &Owner) -> diesel::QueryResult<usize> {
        sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
            .bind::<Text, _>(&owner.key).execute(&mut self.connection)
    }

    fn child_position_checks(&mut self) -> TestResult<Vec<String>> {
        let trigger = sql_query("SELECT sql FROM sqlite_schema WHERE type='trigger' AND name='no_intro_dat_source_positions_publication'")
            .get_result::<SchemaSql>(&mut self.connection)?;
        let queries: Vec<_> = trigger
            .sql
            .split(") OR EXISTS (")
            .filter(|clause| {
                clause.contains("FROM no_intro_dat_game_child_source_positions AS position")
            })
            .map(|clause| {
                let start = clause
                    .find("SELECT 1 FROM")
                    .ok_or("missing child-position SELECT")?;
                Ok(clause[start..].trim().replace("NEW.snapshot_key", "?"))
            })
            .collect::<TestResult<_>>()?;
        assert_eq!(queries.len(), 2, "collision and family-order checks");
        Ok(queries)
    }

    fn assert_snapshot_bounded_child_checks(&mut self, key: &str) -> TestResult {
        for query in self.child_position_checks()? {
            let plan = sql_query(format!("EXPLAIN QUERY PLAN {query}"))
                .bind::<Text, _>(key)
                .load::<QueryPlan>(&mut self.connection)?;
            let details: Vec<_> = plan.into_iter().map(|row| row.detail).collect();
            assert!(
                details.iter().all(|detail| ![
                    "SCAN no_intro_dat_",
                    "SCAN child",
                    "SCAN occurrence",
                    "SCAN owner",
                    "SCAN grouping",
                ]
                .iter()
                .any(|scan| detail.starts_with(scan))),
                "child-position checks must seek snapshot-owned rows, not scan native collections: {details:?}"
            );
            let violations = sql_query(format!("SELECT COUNT(*) AS value FROM ({query})"))
                .bind::<Text, _>(key)
                .get_result::<Id>(&mut self.connection)?
                .value;
            assert_eq!(
                violations, 0,
                "unrelated positions must not affect the target"
            );
        }
        Ok(())
    }

    fn header_options(
        &mut self,
        owner: &mut Owner,
        clr_order: i64,
        romcenter_order: i64,
    ) -> TestResult {
        sql_query("INSERT INTO no_intro_dat_clrmamepro_options(snapshot_key,source_order,source_line,source_column) VALUES(?,?,5,1)")
            .bind::<Text, _>(&owner.key).bind::<BigInt, _>(clr_order).execute(&mut self.connection)?;
        sql_query("INSERT INTO no_intro_dat_romcenter_options(snapshot_key,source_order,source_line,source_column) VALUES(?,?,6,1)")
            .bind::<Text, _>(&owner.key).bind::<BigInt, _>(romcenter_order).execute(&mut self.connection)?;
        owner.counts.clr_options = 1;
        owner.counts.romcenter_options = 1;
        Ok(())
    }
}

#[test]
fn child_position_checks_use_snapshot_bounded_query_plans() -> TestResult {
    let mut fixture = Fixture::new()?;
    let owner = fixture.owner("target", 2)?;
    fixture.assert_snapshot_bounded_child_checks(&owner.key)?;
    Ok(())
}

#[test]
fn twenty_thousand_unrelated_categories_do_not_expand_child_position_scans() -> TestResult {
    let mut fixture = Fixture::new()?;
    let owner = fixture.owner("target", 2)?;
    let unrelated = fixture.owner("unrelated", 3)?;
    sql_query("WITH RECURSIVE ranks(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM ranks WHERE n<19999) INSERT INTO no_intro_dat_categories(set_id,category_order,category,source_order,source_line,source_column) SELECT ?,n,'Unrelated',n+1,60,1 FROM ranks")
        .bind::<BigInt, _>(unrelated.game).execute(&mut fixture.connection)?;
    let inserted =
        sql_query("SELECT COUNT(*) AS value FROM no_intro_dat_categories WHERE set_id=?")
            .bind::<BigInt, _>(unrelated.game)
            .get_result::<Id>(&mut fixture.connection)?
            .value;
    assert_eq!(inserted, 20_000);
    fixture.assert_snapshot_bounded_child_checks(&owner.key)?;
    Ok(())
}

#[test]
fn fresh_children_cannot_be_appended_to_a_published_game() -> TestResult {
    for child in [Child::Category, Child::Identifier, Child::Release] {
        let mut fixture = Fixture::new()?;
        let mut owner = fixture.owner("published", 2)?;
        fixture.seal(&owner)?;
        fixture.publish(&owner)?;
        assert!(fixture.child(&mut owner, child, 8).is_err(), "{child:?}");
    }
    Ok(())
}

#[test]
fn child_insert_guards_require_the_native_game_even_without_foreign_key_enforcement() -> TestResult
{
    for child in [Child::Category, Child::Identifier, Child::Release] {
        let mut fixture = Fixture::new()?;
        let mut owner = fixture.owner("missing-native-game", 2)?;
        owner.game = sql_query(
            "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
             VALUES(?,'no_intro_dat_game',1,'Missing native payload',80,1) RETURNING set_id AS value",
        )
        .bind::<BigInt, _>(owner.group)
        .get_result::<Id>(&mut fixture.connection)?
        .value;
        fixture
            .connection
            .batch_execute("PRAGMA foreign_keys=OFF")?;
        assert!(fixture.child(&mut owner, child, 8).is_err(), "{child:?}");
    }
    Ok(())
}

#[test]
fn compatible_linked_roms_and_non_source_contradictions_can_publish() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut known = fixture.owner("known", 2)?;
    let occurrence = fixture.rom(
        &mut known,
        4,
        Some(4),
        Some(CONTENT_UUID),
        &[Hash::Sha1(1), Hash::Md5(2)],
    )?;
    let unrelated = fixture.digest("md5", &[3; 16])?;
    fixture.assertion(occurrence, unrelated, "whole_file", "computed")?;
    fixture.assertion(occurrence, unrelated, "unknown", "unknown")?;
    fixture.seal(&known)?;
    fixture.publish(&known)?;
    let mut incoming = fixture.owner("incoming", 5)?;
    fixture.rom(
        &mut incoming,
        7,
        Some(4),
        Some(CONTENT_UUID),
        &[Hash::Sha1(1), Hash::Md5(2)],
    )?;
    fixture.seal(&incoming)?;
    fixture.publish(&incoming)?;
    Ok(())
}

#[test]
fn linked_rom_publication_rejects_contradictory_hashes_and_whole_file_sizes() -> TestResult {
    for (size, hashes) in [
        (
            5,
            vec![Hash::Sha1(1), Hash::Md5(2), Hash::Crc(3), Hash::Sha256(4)],
        ),
        (4, vec![Hash::Sha1(1), Hash::Md5(9)]),
        (4, vec![Hash::Sha1(1), Hash::Crc(9)]),
        (4, vec![Hash::Sha1(9)]),
        (4, vec![Hash::Sha1(1), Hash::Sha256(9)]),
    ] {
        let mut fixture = Fixture::new()?;
        let mut known = fixture.owner("known", 2)?;
        fixture.rom(
            &mut known,
            4,
            Some(4),
            Some(CONTENT_UUID),
            &[Hash::Sha1(1), Hash::Md5(2), Hash::Crc(3), Hash::Sha256(4)],
        )?;
        fixture.seal(&known)?;
        fixture.publish(&known)?;
        let mut incoming = fixture.owner("incoming", 3)?;
        fixture.rom(&mut incoming, 7, Some(size), Some(CONTENT_UUID), &hashes)?;
        fixture.seal(&incoming)?;
        assert!(
            fixture.publish(&incoming).is_err(),
            "size={size}, hashes={hashes:?}"
        );
    }
    Ok(())
}

#[test]
fn linked_rom_source_assertion_rejects_an_already_unresolved_hash_dispute() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut known = fixture.owner("known", 2)?;
    fixture.rom(&mut known, 4, Some(4), Some(CONTENT_UUID), &[Hash::Sha1(1)])?;
    fixture.seal(&known)?;
    fixture.publish(&known)?;
    let mut disputed = fixture.owner("disputed", 2)?;
    let occurrence = fixture.rom(&mut disputed, 4, Some(5), None, &[Hash::Sha1(1)])?;
    let digest = fixture.digest("sha1", &[1; 20])?;
    sql_query("INSERT INTO occurrence_content_conflicts(occurrence_id,candidate_content_uuid,reason) VALUES(?,?,'contradictory_assertions')")
        .bind::<BigInt, _>(occurrence).bind::<Binary, _>(CONTENT_UUID).execute(&mut fixture.connection)?;
    sql_query("INSERT INTO occurrence_content_conflict_hashes(occurrence_id,candidate_content_uuid,evidence_occurrence_id,digest_id,scope,provenance,role) VALUES(?,?,?,?,'whole_file','source_declared','incoming')")
        .bind::<BigInt, _>(occurrence).bind::<Binary, _>(CONTENT_UUID).bind::<BigInt, _>(occurrence)
        .bind::<BigInt, _>(digest).execute(&mut fixture.connection)?;
    fixture.seal(&disputed)?;
    fixture.publish(&disputed)?;
    let mut incoming = fixture.owner("incoming", 2)?;
    let error = fixture
        .rom(
            &mut incoming,
            4,
            Some(4),
            Some(CONTENT_UUID),
            &[Hash::Sha1(1)],
        )
        .expect_err("a linked claim cannot supply an already disputed source hash");
    assert!(error.to_string().contains("already disputed source hash"));
    Ok(())
}

#[test]
fn digest_assertions_cannot_precede_their_owner_even_when_foreign_keys_are_deferred() -> TestResult
{
    let mut fixture = Fixture::new()?;
    fixture
        .connection
        .batch_execute("BEGIN; PRAGMA defer_foreign_keys=ON;")?;
    let digest = fixture.digest("sha1", &[1; 20])?;
    let error = fixture
        .assertion(99, digest, "whole_file", "source_declared")
        .expect_err("deferred foreign keys cannot bypass assertion owner checks");
    assert!(error.to_string().contains("allocated occurrence owner"));
    fixture.connection.batch_execute("ROLLBACK;")?;
    Ok(())
}

#[test]
fn source_position_gaps_and_separate_attribute_child_domains_are_valid() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut owner = fixture.owner_with_header_order("gaps", 3, 7)?;
    fixture.header_options(&mut owner, 5, 9)?;
    for (child, order) in [
        (Child::Category, 2),
        (Child::Identifier, 5),
        (Child::Rom, 9),
        (Child::Release, 15),
    ] {
        fixture.child(&mut owner, child, order)?;
    }
    fixture.child(&mut owner, Child::Category, 22)?;
    fixture.seal(&owner)?;
    fixture.publish(&owner)?;
    Ok(())
}

#[test]
fn root_children_cannot_share_or_reverse_source_positions() -> TestResult {
    for orders in [vec![0], vec![2, 2], vec![8, 3]] {
        let mut fixture = Fixture::new()?;
        let mut orders = orders.into_iter();
        let mut owner = fixture.owner("root", orders.next().ok_or("missing root order")?)?;
        for order in orders {
            fixture.game(&mut owner, order)?;
        }
        fixture.seal(&owner)?;
        assert!(fixture.publish(&owner).is_err());
    }
    Ok(())
}

#[test]
fn header_must_precede_games_even_without_a_root_position_collision() -> TestResult {
    let mut fixture = Fixture::new()?;
    let owner = fixture.owner_with_header_order("late-header", 7, 2)?;
    fixture.seal(&owner)?;
    assert!(fixture.publish(&owner).is_err());
    Ok(())
}

#[test]
fn header_fields_and_option_elements_cannot_share_source_positions() -> TestResult {
    for (clr, romcenter) in [(1, 5), (4, 4)] {
        let mut fixture = Fixture::new()?;
        let mut owner = fixture.owner("header", 2)?;
        fixture.header_options(&mut owner, clr, romcenter)?;
        fixture.seal(&owner)?;
        assert!(fixture.publish(&owner).is_err());
    }
    Ok(())
}

#[test]
fn game_children_from_different_families_cannot_share_source_positions() -> TestResult {
    let children = [
        Child::Category,
        Child::Identifier,
        Child::Rom,
        Child::Release,
    ];
    for (index, child) in children.iter().copied().enumerate() {
        let mut fixture = Fixture::new()?;
        let mut owner = fixture.owner("description", 2)?;
        fixture.child(&mut owner, child, 0)?;
        fixture.seal(&owner)?;
        assert!(fixture.publish(&owner).is_err(), "description/{child:?}");
        for other in children.iter().copied().skip(index + 1) {
            let mut fixture = Fixture::new()?;
            let mut owner = fixture.owner("children", 2)?;
            fixture.child(&mut owner, child, 7)?;
            fixture.child(&mut owner, other, 7)?;
            fixture.seal(&owner)?;
            assert!(fixture.publish(&owner).is_err(), "{child:?}/{other:?}");
        }
    }
    Ok(())
}

#[test]
fn repeated_child_family_ranks_must_increase_with_source_positions() -> TestResult {
    for child in [
        Child::Category,
        Child::Identifier,
        Child::Rom,
        Child::Release,
    ] {
        let mut fixture = Fixture::new()?;
        let mut owner = fixture.owner("family", 2)?;
        fixture.child(&mut owner, child, 10)?;
        fixture.child(&mut owner, child, 3)?;
        fixture.seal(&owner)?;
        assert!(fixture.publish(&owner).is_err(), "{child:?}");
    }
    Ok(())
}

#[test]
fn release_name_and_region_cannot_claim_the_same_attribute_position() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut owner = fixture.owner("release", 2)?;
    fixture.release(&mut owner, 7, 0, 0)?;
    fixture.seal(&owner)?;
    assert!(fixture.publish(&owner).is_err());
    Ok(())
}
