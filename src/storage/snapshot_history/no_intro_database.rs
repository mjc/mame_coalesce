//! Native No-Intro database-export snapshot-history facts.

use std::collections::{BTreeMap, HashMap};

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Bool, Nullable, Text},
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::domain::SnapshotKey;

#[derive(QueryableByName)]
struct ExportRow {
    #[diesel(sql_type = Text)]
    envelope_kind: String,
    #[diesel(sql_type = Bool)]
    header_present: bool,
}

#[derive(QueryableByName)]
struct HeaderPresenceRow {
    #[diesel(sql_type = Bool)]
    present: bool,
}

#[derive(QueryableByName)]
struct HeaderFieldRow {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct GameRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
}

#[derive(QueryableByName)]
struct SiblingRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    kind: i64,
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
}

#[derive(Clone, Copy)]
enum ChildKind {
    Archive,
    DumpSource,
    Release,
}

impl TryFrom<i64> for ChildKind {
    type Error = crate::Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Archive),
            1 => Ok(Self::DumpSource),
            2 => Ok(Self::Release),
            _ => Err(schema_error("invalid No-Intro sibling kind")),
        }
    }
}

#[derive(QueryableByName, Serialize)]
struct ArchiveRow {
    #[serde(skip)]
    #[diesel(sql_type = BigInt)]
    archive_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    additional: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    adult: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    aftermarket: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    alt: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    bios: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    categories: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    complete: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dat: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    datter_note: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    devstatus: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    gameid1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    gameid2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    langchecked: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    languages: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    licensed: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    listed: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mergename: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name_alt: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    number: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    physical: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    regparent: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    showlang: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    special1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    special2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sticky_note: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version2: Option<String>,
}

#[derive(QueryableByName)]
struct ArchivePositionRow {
    #[diesel(sql_type = BigInt)]
    archive_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
}

#[derive(QueryableByName)]
struct ArchiveReferenceRow {
    #[diesel(sql_type = BigInt)]
    archive_id: i64,
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct NativePositionRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
}

#[derive(Clone)]
struct OrderedFact {
    source_order: i64,
    value: Value,
}

trait NativeOwnerRow: Serialize {
    fn owner_id(&self) -> i64;
    fn source_order(&self) -> i64;
}

macro_rules! native_owner_row {
    ($name:ident, $owner:ident, { $($field:ident : $ty:ty),+ $(,)? }) => {
        #[derive(QueryableByName, Serialize)]
        struct $name {
            #[serde(skip)]
            #[diesel(sql_type = BigInt)]
            $owner: i64,
            #[serde(skip)]
            #[diesel(sql_type = BigInt)]
            source_order: i64,
            $(#[diesel(sql_type = Nullable<Text>)] $field: Option<$ty>,)+
        }

        impl NativeOwnerRow for $name {
            fn owner_id(&self) -> i64 { self.$owner }
            fn source_order(&self) -> i64 { self.source_order }
        }
    };
}

native_owner_row!(DumpDetailsRow, dump_source_id, {
    comment1: String, comment2: String, d_date: String, d_date_info: String,
    dumper: String, id: String, link1: String, link2: String, link3: String,
    media_title: String, nodump: String, origin: String, originalformat: String,
    project: String, r_date: String, r_date_info: String, region: String,
    rominfo: String, section: String, tool: String
});
native_owner_row!(DumpSerialsRow, dump_source_id, {
    box_barcode: String, box_serial: String, chip_serial: String,
    digital_serial1: String, digital_serial2: String, lockout_serial: String,
    media_serial1: String, media_serial2: String, media_serial3: String,
    mediastamp: String, pcb_serial: String, romchip_serial1: String,
    romchip_serial2: String, savechip_serial: String
});
native_owner_row!(ReleaseDetailsRow, release_id, {
    archivename: String, category: String, comment: String, date: String,
    dirname: String, group: String, id: String, nfo_size: String,
    nfoname: String, nfosize: String, origin: String, originalformat: String,
    region: String, rominfo: String, tool: String
});
native_owner_row!(ReleaseSerialsRow, release_id, {
    box_barcode: String, box_serial: String, media_serial1: String,
    mediastamp: String, pcb_serial: String, romchip_serial1: String
});

#[derive(QueryableByName)]
struct DumpSourceRow {
    #[diesel(sql_type = BigInt)]
    dump_source_id: i64,
}

#[derive(QueryableByName)]
struct ReleaseRow {
    #[diesel(sql_type = BigInt)]
    release_id: i64,
}

#[derive(QueryableByName, Serialize)]
struct FileRow {
    #[serde(skip)]
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[serde(skip)]
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[serde(skip)]
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    evidence_provenance: String,
    #[diesel(sql_type = Nullable<Text>)]
    bad: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    extension: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    filter: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcename: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcescenename: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    format: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    item: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mia: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    note: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    origin_size: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[serde(rename = "size")]
    #[diesel(sql_type = Nullable<Text>)]
    source_size: Option<String>,
    #[serde(rename = "unique")]
    #[diesel(sql_type = Nullable<Text>)]
    unique_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    update_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
}

#[derive(QueryableByName)]
struct DigestRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    digest: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    invalid_literal: Option<String>,
}

#[derive(QueryableByName)]
struct NfoDigestRow {
    #[diesel(sql_type = BigInt)]
    release_id: i64,
    #[diesel(sql_type = Text)]
    source_hash_field: String,
    #[diesel(sql_type = Nullable<Binary>)]
    digest: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    invalid_literal: Option<String>,
    #[diesel(sql_type = Text)]
    presence: String,
    #[diesel(sql_type = Text)]
    scope: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FileOwner {
    DumpSource,
    Release,
}

impl FileOwner {
    const fn table(self) -> &'static str {
        match self {
            Self::DumpSource => "no_intro_dump_files",
            Self::Release => "no_intro_release_files",
        }
    }

    const fn parent_table(self) -> &'static str {
        match self {
            Self::DumpSource => "no_intro_dump_sources",
            Self::Release => "no_intro_releases",
        }
    }

    const fn parent_column(self) -> &'static str {
        match self {
            Self::DumpSource => "dump_source_id",
            Self::Release => "release_id",
        }
    }

    const fn positions_table(self) -> &'static str {
        match self {
            Self::DumpSource => "no_intro_dump_file_field_positions",
            Self::Release => "no_intro_release_file_field_positions",
        }
    }

    const fn digests_table(self) -> &'static str {
        match self {
            Self::DumpSource => "no_intro_dump_file_digests",
            Self::Release => "no_intro_release_file_digests",
        }
    }

    const fn is_dump(self) -> bool {
        matches!(self, Self::DumpSource)
    }
}

/// Load the envelope, header presence and repeated ordered header fields.
pub(super) fn load_document(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<Option<Value>> {
    let export = sql_query(
        "SELECT envelope_kind, header_present FROM no_intro_exports WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .get_result::<ExportRow>(conn)
    .optional()?;
    let Some(export) = export else {
        return Ok(None);
    };
    let header_present =
        sql_query("SELECT 1 AS present FROM no_intro_export_headers WHERE snapshot_key = ?")
            .bind::<Text, _>(key.as_str())
            .get_result::<HeaderPresenceRow>(conn)
            .optional()?
            .is_some_and(|row| row.present);
    let fields = sql_query(
        "SELECT field_kind, value FROM no_intro_header_fields \
         WHERE snapshot_key = ? ORDER BY source_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<HeaderFieldRow>(conn)?
    .into_iter()
    .map(|row| {
        Ok(json!({
            "field": header_field_name(row.field_kind)?,
            "value": row.value,
        }))
    })
    .collect::<crate::Result<Vec<_>>>()?;
    Ok(Some(json!({
        "envelope_kind": export.envelope_kind,
        "header_present": export.header_present,
        "header": header_present.then(|| json!({"fields": fields})),
    })))
}

/// Load ordered native game facts. Map keys are local joins and are not facts.
pub(super) fn load_games(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<Value>>> {
    let games = sql_query(
        "SELECT sets.set_id, sets.set_name FROM catalog_sets AS sets \
         JOIN catalog_set_groups AS groups USING (set_group_id) \
         JOIN no_intro_database_games AS games USING (set_id) \
         WHERE groups.snapshot_key = ? ORDER BY sets.list_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<GameRow>(conn)?;
    let siblings = sql_query(
        "SELECT siblings.set_id, siblings.source_order, siblings.kind, siblings.owner_id \
         FROM ( \
           SELECT archive.set_id, archive.source_order, 0 AS kind, archive.archive_id AS owner_id \
             FROM no_intro_archive_descriptions AS archive JOIN catalog_sets AS sets USING (set_id) \
             JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ? \
           UNION ALL \
           SELECT source.set_id, source.source_order, 1 AS kind, source.dump_source_id AS owner_id \
             FROM no_intro_dump_sources AS source JOIN catalog_sets AS sets USING (set_id) \
             JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ? \
           UNION ALL \
           SELECT release.set_id, release.source_order, 2 AS kind, release.release_id AS owner_id \
             FROM no_intro_releases AS release JOIN catalog_sets AS sets USING (set_id) \
             JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ? \
         ) AS siblings ORDER BY siblings.set_id, siblings.source_order",
    )
    .bind::<Text, _>(key.as_str())
    .bind::<Text, _>(key.as_str())
    .bind::<Text, _>(key.as_str())
    .load::<SiblingRow>(conn)?;

    let archives = load_archives(conn, key)?;
    let dump_sources = load_dump_sources(conn, key)?;
    let releases = load_releases(conn, key)?;
    let mut ordered = BTreeMap::<i64, Vec<Value>>::new();
    for sibling in siblings {
        let fact = match ChildKind::try_from(sibling.kind)? {
            ChildKind::Archive => archives.get(&sibling.owner_id),
            ChildKind::DumpSource => dump_sources.get(&sibling.owner_id),
            ChildKind::Release => releases.get(&sibling.owner_id),
        }
        .ok_or_else(|| schema_error("No-Intro sibling owner is missing"))?
        .clone();
        ordered.entry(sibling.set_id).or_default().push(fact);
    }
    Ok(games
        .into_iter()
        .map(|game| {
            let children = ordered.remove(&game.set_id).unwrap_or_default();
            (
                game.set_id,
                vec![json!({"name": game.set_name, "children": children})],
            )
        })
        .collect())
}

fn load_archives(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<i64, Value>> {
    let rows = sql_query(
        "SELECT archive.archive_id, archive.set_id, archive.source_order, archive.additional, archive.adult, \
                archive.aftermarket, archive.alt, archive.bios, archive.categories, archive.complete, archive.dat, \
                archive.datter_note, archive.description, archive.devstatus, archive.gameid1, archive.gameid2, \
                archive.langchecked, archive.languages, archive.licensed, archive.listed, archive.mergename, \
                archive.name, archive.name_alt, archive.number, archive.physical, archive.region, archive.regparent, \
                archive.showlang, archive.special1, archive.special2, archive.sticky_note, archive.version1, archive.version2 \
         FROM no_intro_archive_descriptions AS archive JOIN catalog_sets AS sets USING (set_id) \
         JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ? \
         ORDER BY archive.set_id, archive.source_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<ArchiveRow>(conn)?;
    let positions = sql_query(
        "SELECT positions.archive_id, positions.field_kind FROM no_intro_archive_field_positions AS positions \
         JOIN no_intro_archive_descriptions AS archive USING (archive_id) JOIN catalog_sets AS sets USING (set_id) \
         JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ? \
         ORDER BY positions.archive_id, positions.source_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<ArchivePositionRow>(conn)?;
    let clone_markers = sql_query(
        "SELECT marker.archive_id, marker.marker AS value FROM no_intro_archive_clone_markers AS marker \
         JOIN no_intro_archive_descriptions AS archive USING (archive_id) JOIN catalog_sets AS sets USING (set_id) \
         JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<ArchiveReferenceRow>(conn)?
    .into_iter()
    .map(|row| (row.archive_id, row.value))
    .collect::<HashMap<_, _>>();
    let clone_refs = load_archive_references(
        conn,
        key,
        "no_intro_archive_clone_links",
        "declared_target_number",
    )?;
    let merge_refs = load_archive_references(
        conn,
        key,
        "no_intro_archive_merge_links",
        "declared_mergeof",
    )?;
    let mut field_orders = HashMap::<i64, Vec<i64>>::new();
    for row in positions {
        field_orders
            .entry(row.archive_id)
            .or_default()
            .push(row.field_kind);
    }
    let mut output = HashMap::new();
    for row in rows {
        let mut values = serde_json::to_value(&row)?;
        let object = values
            .as_object_mut()
            .ok_or_else(|| schema_error("typed archive row did not serialize as an object"))?;
        object.insert(
            "clone".into(),
            clone_markers
                .get(&row.archive_id)
                .or_else(|| clone_refs.get(&row.archive_id))
                .map_or(Value::Null, |value| json!(value)),
        );
        object.insert(
            "mergeof".into(),
            merge_refs
                .get(&row.archive_id)
                .map_or(Value::Null, |value| json!(value)),
        );
        let order = field_orders.remove(&row.archive_id).unwrap_or_default();
        let fields = order
            .into_iter()
            .map(|kind| archive_field_name(kind).map(Value::from))
            .collect::<crate::Result<Vec<_>>>()?;
        output.insert(
            row.archive_id,
            json!({"kind": "archive", "values": values, "field_order": fields}),
        );
    }
    Ok(output)
}

fn load_archive_references(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    table: &'static str,
    value_column: &'static str,
) -> crate::Result<HashMap<i64, String>> {
    let query = format!(
        "SELECT reference.archive_id, reference.{value_column} AS value FROM {table} AS reference \
         JOIN no_intro_archive_descriptions AS archive USING (archive_id) JOIN catalog_sets AS sets USING (set_id) \
         JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ?"
    );
    Ok(sql_query(query)
        .bind::<Text, _>(key.as_str())
        .load::<ArchiveReferenceRow>(conn)?
        .into_iter()
        .map(|row| (row.archive_id, row.value))
        .collect())
}

fn load_dump_sources(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<i64, Value>> {
    let owners = sql_query(
        "SELECT source.dump_source_id, source.set_id FROM no_intro_dump_sources AS source \
         JOIN catalog_sets AS sets USING (set_id) JOIN catalog_set_groups AS groups USING (set_group_id) \
         WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<DumpSourceRow>(conn)?;
    let details = load_dump_details(conn, key)?;
    let serials = load_dump_serials(conn, key)?;
    let files = load_files(conn, key, FileOwner::DumpSource)?;
    Ok(assemble_events(
        owners.into_iter().map(|row| row.dump_source_id),
        details,
        serials,
        files,
        "source",
    ))
}

fn load_releases(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<i64, Value>> {
    let owners = sql_query(
        "SELECT release.release_id, release.set_id FROM no_intro_releases AS release \
         JOIN catalog_sets AS sets USING (set_id) JOIN catalog_set_groups AS groups USING (set_group_id) \
         WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<ReleaseRow>(conn)?;
    let details = load_release_details(conn, key)?;
    let serials = load_release_serials(conn, key)?;
    let files = load_files(conn, key, FileOwner::Release)?;
    Ok(assemble_events(
        owners.into_iter().map(|row| row.release_id),
        details,
        serials,
        files,
        "release",
    ))
}

fn assemble_events(
    owner_ids: impl Iterator<Item = i64>,
    mut details: HashMap<i64, OrderedFact>,
    mut serials: HashMap<i64, OrderedFact>,
    mut files: HashMap<i64, Vec<OrderedFact>>,
    kind: &'static str,
) -> HashMap<i64, Value> {
    let mut output = HashMap::new();
    for owner_id in owner_ids {
        let detail = details.remove(&owner_id);
        let serial = serials.remove(&owner_id);
        let owner_files = files.remove(&owner_id).unwrap_or_default();
        let mut ordered = Vec::<OrderedFact>::new();
        if let Some(fact) = &detail {
            ordered.push(fact.clone());
        }
        if let Some(fact) = &serial {
            ordered.push(fact.clone());
        }
        ordered.extend(owner_files);
        ordered.sort_by_key(|fact| fact.source_order);
        let children = ordered
            .into_iter()
            .map(|fact| fact.value)
            .collect::<Vec<_>>();
        output.insert(
            owner_id,
            json!({
                "kind": kind,
                "details": detail.map(|fact| fact.value),
                "serials": serial.map(|fact| fact.value),
                "children": children,
            }),
        );
    }
    output
}

fn load_dump_details(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<i64, OrderedFact>> {
    let rows = sql_query(
        "SELECT details.dump_source_id, details.source_order, details.comment1, details.comment2, \
                details.d_date, details.d_date_info, details.dumper, details.id, details.link1, details.link2, \
                details.link3, details.media_title, details.nodump, details.origin, details.originalformat, \
                details.project, details.r_date, details.r_date_info, details.region, details.rominfo, details.section, details.tool \
         FROM no_intro_dump_details AS details JOIN no_intro_dump_sources AS source USING (dump_source_id) \
         JOIN catalog_sets AS sets USING (set_id) JOIN catalog_set_groups AS groups USING (set_group_id) \
         WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<DumpDetailsRow>(conn)?;
    let positions = load_positions(
        conn,
        key,
        "no_intro_dump_details_field_positions",
        "dump_source_id",
        "no_intro_dump_sources",
        "dump_source_id",
    )?;
    map_ordered_owners(rows, positions, dump_details_field_name, "details")
}

fn load_dump_serials(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<i64, OrderedFact>> {
    let rows = sql_query(
        "SELECT serials.dump_source_id, serials.source_order, serials.box_barcode, serials.box_serial, \
                serials.chip_serial, serials.digital_serial1, serials.digital_serial2, serials.lockout_serial, \
                serials.media_serial1, serials.media_serial2, serials.media_serial3, serials.mediastamp, \
                serials.pcb_serial, serials.romchip_serial1, serials.romchip_serial2, serials.savechip_serial \
         FROM no_intro_dump_serials AS serials JOIN no_intro_dump_sources AS source USING (dump_source_id) \
         JOIN catalog_sets AS sets USING (set_id) JOIN catalog_set_groups AS groups USING (set_group_id) \
         WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<DumpSerialsRow>(conn)?;
    let positions = load_positions(
        conn,
        key,
        "no_intro_dump_serials_field_positions",
        "dump_source_id",
        "no_intro_dump_sources",
        "dump_source_id",
    )?;
    map_ordered_owners(rows, positions, dump_serials_field_name, "serials")
}

fn load_release_details(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<i64, OrderedFact>> {
    let rows = sql_query(
        "SELECT details.release_id, details.source_order, details.archivename, details.category, details.comment, \
                details.date, details.dirname, details.\"group\" AS \"group\", details.id, details.nfo_size, \
                details.nfoname, details.nfosize, details.origin, details.originalformat, details.region, \
                details.rominfo, details.tool \
         FROM no_intro_release_details AS details JOIN no_intro_releases AS release USING (release_id) \
         JOIN catalog_sets AS sets USING (set_id) JOIN catalog_set_groups AS groups USING (set_group_id) \
         WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<ReleaseDetailsRow>(conn)?;
    let positions = load_positions(
        conn,
        key,
        "no_intro_release_details_field_positions",
        "release_id",
        "no_intro_releases",
        "release_id",
    )?;
    let nfo = load_nfo_digests(conn, key)?;
    map_release_detail_owners(rows, positions, &nfo)
}

fn load_release_serials(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<i64, OrderedFact>> {
    let rows = sql_query(
        "SELECT serials.release_id, serials.source_order, serials.box_barcode, serials.box_serial, \
                serials.media_serial1, serials.mediastamp, serials.pcb_serial, serials.romchip_serial1 \
         FROM no_intro_release_serials AS serials JOIN no_intro_releases AS release USING (release_id) \
         JOIN catalog_sets AS sets USING (set_id) JOIN catalog_set_groups AS groups USING (set_group_id) \
         WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<ReleaseSerialsRow>(conn)?;
    let positions = load_positions(
        conn,
        key,
        "no_intro_release_serials_field_positions",
        "release_id",
        "no_intro_releases",
        "release_id",
    )?;
    map_ordered_owners(rows, positions, release_serials_field_name, "serials")
}

fn map_ordered_owners<R: NativeOwnerRow>(
    rows: Vec<R>,
    mut positions: HashMap<i64, Vec<i64>>,
    field_name: fn(i64) -> crate::Result<&'static str>,
    owner_kind: &'static str,
) -> crate::Result<HashMap<i64, OrderedFact>> {
    let mut result = HashMap::new();
    for row in rows {
        let serialized = serde_json::to_value(&row)?;
        let values = serialized.as_object().ok_or_else(|| {
            schema_error("typed No-Intro owner row did not serialize as an object")
        })?;
        let fields = positions
            .remove(&row.owner_id())
            .unwrap_or_default()
            .into_iter()
            .map(|kind| {
                let name = field_name(kind)?;
                let value = values.get(name).ok_or_else(|| {
                    schema_error("No-Intro field map does not match its typed row")
                })?;
                Ok(json!({"field": name, "value": value}))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        result.insert(
            row.owner_id(),
            OrderedFact {
                source_order: row.source_order(),
                value: json!({"kind": owner_kind, "fields": fields}),
            },
        );
    }
    Ok(result)
}

fn map_release_detail_owners(
    rows: Vec<ReleaseDetailsRow>,
    mut positions: HashMap<i64, Vec<i64>>,
    nfo_digests: &HashMap<(i64, String), Value>,
) -> crate::Result<HashMap<i64, OrderedFact>> {
    let mut result = HashMap::new();
    for row in rows {
        let serialized = serde_json::to_value(&row)?;
        let values = serialized.as_object().ok_or_else(|| {
            schema_error("typed release-details row did not serialize as an object")
        })?;
        let fields = positions
            .remove(&row.release_id)
            .unwrap_or_default()
            .into_iter()
            .map(|kind| {
                let name = release_details_field_name(kind)?;
                let value = match name {
                    "nfo_crc32" | "nfocrc" => nfo_digests
                        .get(&(row.release_id, name.to_owned()))
                        .cloned()
                        .ok_or_else(|| {
                            schema_error("NFO digest field position has no native hash owner")
                        })?,
                    _ => values.get(name).cloned().ok_or_else(|| {
                        schema_error("release-details field map does not match its typed row")
                    })?,
                };
                Ok(json!({"field": name, "value": value}))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        result.insert(
            row.release_id,
            OrderedFact {
                source_order: row.source_order,
                value: json!({"kind": "details", "fields": fields}),
            },
        );
    }
    Ok(result)
}

fn load_nfo_digests(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<HashMap<(i64, String), Value>> {
    let rows = sql_query(
        "SELECT hashes.release_id, hashes.source_hash_field, digest_values.digest, hashes.invalid_literal, \
                hashes.presence, hashes.scope \
         FROM no_intro_release_nfo_hashes AS hashes LEFT JOIN digest_values \
         ON digest_values.digest_id = hashes.hash_id \
         JOIN no_intro_release_details AS details USING (release_id) \
         JOIN no_intro_releases AS release USING (release_id) JOIN catalog_sets AS sets USING (set_id) \
         JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NfoDigestRow>(conn)?;
    rows.into_iter()
        .map(|row| {
            let value = digest_literal(row.digest, row.invalid_literal)?;
            Ok((
                (row.release_id, row.source_hash_field.clone()),
                json!({
                    "present": row.presence,
                    "field_kind": row.source_hash_field,
                    "scope": row.scope,
                    "provenance": "source_declared",
                    "value": value,
                }),
            ))
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum FileQuery {
    Rows,
    Positions,
    Digests,
}

impl FileOwner {
    fn query(self, kind: FileQuery) -> String {
        let table = self.table();
        let owner_table = self.parent_table();
        let owner_column = self.parent_column();
        let positions_table = self.positions_table();
        let digest_table = self.digests_table();
        let is_dump = self.is_dump();
        let optional_source_fields = if is_dump {
            "file.date, file.filter, file.mia, file.origin_size, file.\"unique\" AS unique_text"
        } else {
            "NULL AS date, NULL AS filter, NULL AS mia, NULL AS origin_size, NULL AS unique_text"
        };
        // Pin the selected snapshot as the outer loop. Reordering these joins
        // lets SQLite scan every catalog's files before applying the scope.
        let scoped_files = format!(
            "FROM catalog_set_groups AS groups \
             CROSS JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id \
             CROSS JOIN {owner_table} AS parent ON parent.set_id = sets.set_id \
             CROSS JOIN {table} AS file ON file.{owner_column} = parent.{owner_column}"
        );

        match kind {
            FileQuery::Rows => format!(
                "SELECT file.occurrence_id, file.{owner_column} AS owner_id, file.source_order, \
                file.evidence_scope, file.evidence_provenance, file.bad, {optional_source_fields}, \
                file.extension, file.forcename, file.forcescenename, file.format, file.header, file.id, \
                file.item, file.note, file.serial, file.source_size, file.update_type, file.version \
         {scoped_files} \
         WHERE groups.snapshot_key = ? ORDER BY file.{owner_column}, file.source_order"
            ),
            FileQuery::Positions => format!(
                "SELECT positions.occurrence_id AS owner_id, positions.field_kind {scoped_files} \
         CROSS JOIN {positions_table} AS positions ON positions.occurrence_id = file.occurrence_id \
         WHERE groups.snapshot_key = ? ORDER BY positions.occurrence_id, positions.source_order"
            ),
            FileQuery::Digests => format!(
                "SELECT hashes.occurrence_id, hashes.field_kind, digest_values.digest, hashes.invalid_literal \
         {scoped_files} \
         CROSS JOIN {digest_table} AS hashes ON hashes.occurrence_id = file.occurrence_id \
         LEFT JOIN digest_values ON digest_values.digest_id = hashes.digest_id \
         WHERE groups.snapshot_key = ?"
            ),
        }
    }
}

fn load_files(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    owner: FileOwner,
) -> crate::Result<HashMap<i64, Vec<OrderedFact>>> {
    let rows = sql_query(owner.query(FileQuery::Rows))
        .bind::<Text, _>(key.as_str())
        .load::<FileRow>(conn)?;
    let position_rows = sql_query(owner.query(FileQuery::Positions))
        .bind::<Text, _>(key.as_str())
        .load::<NativePositionRow>(conn)?;
    let digest_rows = sql_query(owner.query(FileQuery::Digests))
        .bind::<Text, _>(key.as_str())
        .load::<DigestRow>(conn)?;
    let digest_values = digest_rows
        .into_iter()
        .map(|row| {
            Ok((
                (row.occurrence_id, row.field_kind),
                digest_literal(row.digest, row.invalid_literal)?,
            ))
        })
        .collect::<crate::Result<HashMap<_, _>>>()?;
    let mut positions = HashMap::<i64, Vec<i64>>::new();
    for row in position_rows {
        positions
            .entry(row.owner_id)
            .or_default()
            .push(row.field_kind);
    }
    let mut result = HashMap::<i64, Vec<OrderedFact>>::new();
    for row in rows {
        let field_kinds = positions.remove(&row.occurrence_id).unwrap_or_default();
        let fields = file_fields(&row, owner, field_kinds, &digest_values)?;
        result.entry(row.owner_id).or_default().push(OrderedFact {
            source_order: row.source_order,
            value: json!({
                "kind": "file",
                "evidence_scope": row.evidence_scope,
                "evidence_provenance": row.evidence_provenance,
                "fields": fields,
            }),
        });
    }
    for facts in result.values_mut() {
        facts.sort_by_key(|fact| fact.source_order);
    }
    Ok(result)
}

fn file_fields(
    row: &FileRow,
    owner: FileOwner,
    field_kinds: Vec<i64>,
    digest_values: &HashMap<(i64, i64), Value>,
) -> crate::Result<Vec<Value>> {
    let serialized = serde_json::to_value(row)?;
    let values = serialized
        .as_object()
        .ok_or_else(|| schema_error("typed No-Intro file row did not serialize as an object"))?;
    field_kinds.into_iter().map(|position_kind| {
        let name = file_field_name(owner, position_kind)?;
        let digest_kind = digest_kind(owner, position_kind);
        let value = if let Some(kind) = digest_kind {
            let digest = digest_values.get(&(row.occurrence_id, kind))
                .ok_or_else(|| schema_error("file digest position has no native digest owner"))?;
            json!({
                "present": true,
                "field_kind": name,
                "scope": if owner == FileOwner::DumpSource && position_kind == 14 { "source_origin" } else { row.evidence_scope.as_str() },
                "provenance": row.evidence_provenance,
                "value": digest,
            })
        } else {
            let value = values.get(name).cloned().ok_or_else(|| schema_error("file field map does not match its typed row"))?;
            if owner == FileOwner::DumpSource && position_kind == 15 {
                json!({"value": value, "scope": "source_origin"})
            } else {
                value
            }
        };
        Ok(json!({"field": name, "value": value}))
    }).collect::<crate::Result<Vec<_>>>()
}

fn load_positions(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    position_table: &'static str,
    position_owner: &'static str,
    owner_table: &'static str,
    owner_column: &'static str,
) -> crate::Result<HashMap<i64, Vec<i64>>> {
    let query = format!(
        "SELECT positions.{position_owner} AS owner_id, positions.field_kind FROM {position_table} AS positions \
         JOIN {owner_table} AS parent USING ({owner_column}) JOIN catalog_sets AS sets USING (set_id) \
         JOIN catalog_set_groups AS groups USING (set_group_id) WHERE groups.snapshot_key = ? \
         ORDER BY positions.{position_owner}, positions.source_order"
    );
    let rows = sql_query(query)
        .bind::<Text, _>(key.as_str())
        .load::<NativePositionRow>(conn)?;
    let mut result = HashMap::<i64, Vec<i64>>::new();
    for row in rows {
        result.entry(row.owner_id).or_default().push(row.field_kind);
    }
    Ok(result)
}

fn header_field_name(kind: i64) -> crate::Result<&'static str> {
    match kind {
        0 => Ok("author"),
        1 => Ok("piracy"),
        2 => Ok("trademarks"),
        3 => Ok("url"),
        4 => Ok("version"),
        _ => Err(schema_error("invalid No-Intro header field kind")),
    }
}

fn archive_field_name(kind: i64) -> crate::Result<&'static str> {
    match kind {
        0 => Ok("additional"),
        1 => Ok("adult"),
        2 => Ok("aftermarket"),
        3 => Ok("alt"),
        4 => Ok("bios"),
        5 => Ok("categories"),
        6 => Ok("complete"),
        7 => Ok("dat"),
        8 => Ok("datter_note"),
        9 => Ok("description"),
        10 => Ok("devstatus"),
        11 => Ok("gameid1"),
        12 => Ok("gameid2"),
        13 => Ok("langchecked"),
        14 => Ok("languages"),
        15 => Ok("licensed"),
        16 => Ok("listed"),
        17 => Ok("mergename"),
        18 => Ok("name"),
        19 => Ok("name_alt"),
        20 => Ok("number"),
        21 => Ok("physical"),
        22 => Ok("region"),
        23 => Ok("regparent"),
        24 => Ok("showlang"),
        25 => Ok("special1"),
        26 => Ok("special2"),
        27 => Ok("sticky_note"),
        28 => Ok("version1"),
        29 => Ok("version2"),
        30 => Ok("clone"),
        31 => Ok("mergeof"),
        _ => Err(schema_error("invalid No-Intro archive field kind")),
    }
}

fn dump_details_field_name(kind: i64) -> crate::Result<&'static str> {
    match kind {
        0 => Ok("comment1"),
        1 => Ok("comment2"),
        2 => Ok("d_date"),
        3 => Ok("d_date_info"),
        4 => Ok("dumper"),
        5 => Ok("id"),
        6 => Ok("link1"),
        7 => Ok("link2"),
        8 => Ok("link3"),
        9 => Ok("media_title"),
        10 => Ok("nodump"),
        11 => Ok("origin"),
        12 => Ok("originalformat"),
        13 => Ok("project"),
        14 => Ok("r_date"),
        15 => Ok("r_date_info"),
        16 => Ok("region"),
        17 => Ok("rominfo"),
        18 => Ok("section"),
        19 => Ok("tool"),
        _ => Err(schema_error("invalid No-Intro dump-details field kind")),
    }
}

fn dump_serials_field_name(kind: i64) -> crate::Result<&'static str> {
    match kind {
        0 => Ok("box_barcode"),
        1 => Ok("box_serial"),
        2 => Ok("chip_serial"),
        3 => Ok("digital_serial1"),
        4 => Ok("digital_serial2"),
        5 => Ok("lockout_serial"),
        6 => Ok("media_serial1"),
        7 => Ok("media_serial2"),
        8 => Ok("media_serial3"),
        9 => Ok("mediastamp"),
        10 => Ok("pcb_serial"),
        11 => Ok("romchip_serial1"),
        12 => Ok("romchip_serial2"),
        13 => Ok("savechip_serial"),
        _ => Err(schema_error("invalid No-Intro dump-serials field kind")),
    }
}

fn release_details_field_name(kind: i64) -> crate::Result<&'static str> {
    match kind {
        0 => Ok("archivename"),
        1 => Ok("category"),
        2 => Ok("comment"),
        3 => Ok("date"),
        4 => Ok("dirname"),
        5 => Ok("group"),
        6 => Ok("id"),
        7 => Ok("nfo_crc32"),
        8 => Ok("nfo_size"),
        9 => Ok("nfocrc"),
        10 => Ok("nfoname"),
        11 => Ok("nfosize"),
        12 => Ok("origin"),
        13 => Ok("originalformat"),
        14 => Ok("region"),
        15 => Ok("rominfo"),
        16 => Ok("tool"),
        _ => Err(schema_error("invalid No-Intro release-details field kind")),
    }
}

fn release_serials_field_name(kind: i64) -> crate::Result<&'static str> {
    match kind {
        0 => Ok("box_barcode"),
        1 => Ok("box_serial"),
        2 => Ok("media_serial1"),
        3 => Ok("mediastamp"),
        4 => Ok("pcb_serial"),
        5 => Ok("romchip_serial1"),
        _ => Err(schema_error("invalid No-Intro release-serials field kind")),
    }
}

fn file_field_name(owner: FileOwner, kind: i64) -> crate::Result<&'static str> {
    match (owner, kind) {
        (FileOwner::DumpSource | FileOwner::Release, 0) => Ok("bad"),
        (FileOwner::DumpSource | FileOwner::Release, 1) => Ok("crc32"),
        (FileOwner::DumpSource, 2) => Ok("date"),
        (FileOwner::DumpSource, 3) | (FileOwner::Release, 2) => Ok("extension"),
        (FileOwner::DumpSource, 4) => Ok("filter"),
        (FileOwner::DumpSource, 5) | (FileOwner::Release, 3) => Ok("forcename"),
        (FileOwner::DumpSource, 6) | (FileOwner::Release, 4) => Ok("forcescenename"),
        (FileOwner::DumpSource, 7) | (FileOwner::Release, 5) => Ok("format"),
        (FileOwner::DumpSource, 8) | (FileOwner::Release, 6) => Ok("header"),
        (FileOwner::DumpSource, 9) | (FileOwner::Release, 7) => Ok("id"),
        (FileOwner::DumpSource, 10) | (FileOwner::Release, 8) => Ok("item"),
        (FileOwner::DumpSource, 11) | (FileOwner::Release, 9) => Ok("md5"),
        (FileOwner::DumpSource, 12) => Ok("mia"),
        (FileOwner::DumpSource, 13) | (FileOwner::Release, 10) => Ok("note"),
        (FileOwner::DumpSource, 14) => Ok("origin_sha256"),
        (FileOwner::DumpSource, 15) => Ok("origin_size"),
        (FileOwner::DumpSource, 16) | (FileOwner::Release, 11) => Ok("serial"),
        (FileOwner::DumpSource, 17) | (FileOwner::Release, 12) => Ok("sha1"),
        (FileOwner::DumpSource, 18) | (FileOwner::Release, 13) => Ok("sha256"),
        (FileOwner::DumpSource, 19) | (FileOwner::Release, 14) => Ok("size"),
        (FileOwner::DumpSource, 20) => Ok("unique"),
        (FileOwner::DumpSource, 21) | (FileOwner::Release, 15) => Ok("update_type"),
        (FileOwner::DumpSource, 22) | (FileOwner::Release, 16) => Ok("version"),
        _ => Err(schema_error("invalid No-Intro file field kind")),
    }
}

const fn digest_kind(owner: FileOwner, position_kind: i64) -> Option<i64> {
    match (owner, position_kind) {
        (FileOwner::DumpSource | FileOwner::Release, 1) => Some(0),
        (FileOwner::DumpSource, 11) | (FileOwner::Release, 9) => Some(1),
        (FileOwner::DumpSource, 17) | (FileOwner::Release, 12) => Some(2),
        (FileOwner::DumpSource, 18) | (FileOwner::Release, 13) => Some(3),
        (FileOwner::DumpSource, 14) => Some(4),
        _ => None,
    }
}

fn digest_literal(digest: Option<Vec<u8>>, invalid: Option<String>) -> crate::Result<Value> {
    match (digest, invalid) {
        (Some(bytes), None) => Ok(json!({"encoding": "hex", "literal": hex::encode(bytes)})),
        (None, Some(literal)) => Ok(json!({"encoding": "invalid_literal", "literal": literal})),
        _ => Err(schema_error(
            "native No-Intro digest must have exactly one value form",
        )),
    }
}

fn schema_error(message: &'static str) -> crate::Error {
    schema_error_owned(message.to_owned())
}

const fn schema_error_owned(message: String) -> crate::Error {
    crate::Error::DatabaseSchema(message)
}

#[cfg(test)]
mod query_plan_tests {
    use super::*;

    #[derive(QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    fn assert_snapshot_scoped(kind: FileQuery) -> crate::Result<()> {
        let database = crate::database::Database::in_memory()?;
        let mut conn = database.pool().get()?;
        for owner in [FileOwner::DumpSource, FileOwner::Release] {
            let plan = sql_query(format!("EXPLAIN QUERY PLAN {}", owner.query(kind)))
                .bind::<Text, _>("selected-snapshot")
                .load::<PlanRow>(&mut conn)?;
            let details = plan.into_iter().map(|row| row.detail).collect::<Vec<_>>();
            assert!(
                details.iter().all(|detail| ![
                    "SCAN groups",
                    "SCAN sets",
                    "SCAN parent",
                    "SCAN file",
                    "SCAN positions",
                    "SCAN hashes",
                ]
                .iter()
                .any(|prefix| detail.starts_with(prefix))),
                "{kind:?} must not scan files from unrelated catalogs: {details:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn native_file_rows_are_snapshot_scoped() -> crate::Result<()> {
        assert_snapshot_scoped(FileQuery::Rows)
    }

    #[test]
    fn native_file_positions_are_snapshot_scoped() -> crate::Result<()> {
        assert_snapshot_scoped(FileQuery::Positions)
    }

    #[test]
    fn native_file_digests_are_snapshot_scoped() -> crate::Result<()> {
        assert_snapshot_scoped(FileQuery::Digests)
    }
}
