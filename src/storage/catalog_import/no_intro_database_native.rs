use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    domain::{CatalogSetId, NoIntroArchiveId, NoIntroArchiveReferenceField, SnapshotKey},
    no_intro_db_xml::{
        ArchiveClone, ArchiveDescription, DatabaseDigest, DatabaseGame, DatabaseRelease,
        DumpSource, EnvelopeKind, HeaderFieldKind, NoIntroDatabaseDocument, ReleaseDetails,
        ReleaseFile, ReleaseSerials, SourceDetails, SourceFile, SourceOrRelease, SourceSerials,
    },
    storage::{
        catalog_content::{ContentDigestAssertions, record_occurrence_digest_assertions},
        catalog_identity::OccurrenceId,
        no_intro_database_fields::{
            NoIntroDatabaseArchiveField, NoIntroDatabaseDumpDetailsField,
            NoIntroDatabaseDumpFileField, NoIntroDatabaseDumpSerialsField,
            NoIntroDatabaseReleaseDetailsField, NoIntroDatabaseReleaseFileField,
            NoIntroDatabaseReleaseSerialsField,
        },
    },
    xml_reader::DeclaredText,
};

use super::reported_relationships::{ReferenceOwner, insert_reference};

#[derive(Clone, Copy)]
struct DumpSourceId(i64);
#[derive(Clone, Copy)]
struct ReleaseId(i64);

#[derive(Clone, Copy)]
enum FileTable {
    Dump,
    Release,
}

impl FileTable {
    const fn digest_table(self) -> &'static str {
        match self {
            Self::Dump => "no_intro_dump_file_digests",
            Self::Release => "no_intro_release_file_digests",
        }
    }
}

/// A closed, compile-time mapping to native columns, never stored key/value data.
enum FieldValue<'a> {
    Text(&'static str, Option<&'a DeclaredText>),
    Digest(Option<&'a DatabaseDigest>),
    Clone(Option<&'a ArchiveClone>),
    Link(Option<&'a DeclaredText>),
}

impl<'a> FieldValue<'a> {
    fn declaration(&self) -> Option<&'a DeclaredText> {
        match self {
            Self::Text(_, value) | Self::Link(value) => *value,
            Self::Digest(value) => value.map(|value| &value.source),
            Self::Clone(value) => value.map(|value| match value {
                ArchiveClone::ParentMarker(text) | ArchiveClone::OtherValue(text) => text,
            }),
        }
    }
}

struct Field<'a> {
    kind: i64,
    value: FieldValue<'a>,
}

macro_rules! field_value {
    ($owner:ident, text, $field:ident, $column:literal) => {
        FieldValue::Text($column, $owner.$field.as_ref())
    };
    ($owner:ident, digest, $field:ident, $column:literal) => {
        FieldValue::Digest($owner.$field.as_ref())
    };
    ($owner:ident, clone, $field:ident, $column:literal) => {
        FieldValue::Clone($owner.$field.as_ref())
    };
    ($owner:ident, link, $field:ident, $column:literal) => {
        FieldValue::Link($owner.$field.as_ref())
    };
}

macro_rules! native_fields {
    ($function:ident, $enum:ident, $model:ty, $count:literal;
     $($variant:ident: $storage:ident $field:ident => $column:literal),+ $(,)?) => {
        const fn $function(owner: &$model) -> [Field<'_>; $count] {
            [$(Field {
                kind: $enum::$variant as i64,
                value: field_value!(owner, $storage, $field, $column),
            }),+]
        }
    };
}

native_fields!(archive_fields, NoIntroDatabaseArchiveField, ArchiveDescription, 32;
    Additional:text additional=>"additional", Adult:text adult=>"adult",
    Aftermarket:text aftermarket=>"aftermarket", Alt:text alt=>"alt", Bios:text bios=>"bios",
    Categories:text categories=>"categories", Complete:text complete=>"complete",
    Dat:text dat=>"dat", DatterNote:text datter_note=>"datter_note",
    Description:text description=>"description", Devstatus:text devstatus=>"devstatus",
    GameId1:text gameid1=>"gameid1", GameId2:text gameid2=>"gameid2",
    Langchecked:text langchecked=>"langchecked", Languages:text languages=>"languages",
    Licensed:text licensed=>"licensed", Listed:text listed=>"listed",
    Mergename:text mergename=>"mergename", Name:text name=>"name",
    NameAlt:text name_alt=>"name_alt", Number:text number=>"number",
    Physical:text physical=>"physical", Region:text region=>"region",
    Regparent:text regparent=>"regparent", Showlang:text showlang=>"showlang",
    Special1:text special1=>"special1", Special2:text special2=>"special2",
    StickyNote:text sticky_note=>"sticky_note", Version1:text version1=>"version1",
    Version2:text version2=>"version2", Clone:clone clone=>"clone",
    MergeOf:link mergeof=>"mergeof"
);

native_fields!(dump_details_fields, NoIntroDatabaseDumpDetailsField, SourceDetails, 20;
    Comment1:text comment1=>"comment1", Comment2:text comment2=>"comment2",
    DumpDate:text d_date=>"d_date", DumpDateInfo:text d_date_info=>"d_date_info",
    Dumper:text dumper=>"dumper", Id:text id=>"id", Link1:text link1=>"link1",
    Link2:text link2=>"link2", Link3:text link3=>"link3", MediaTitle:text media_title=>"media_title",
    NoDump:text nodump=>"nodump", Origin:text origin=>"origin",
    OriginalFormat:text originalformat=>"originalformat", Project:text project=>"project",
    ReleaseDate:text r_date=>"r_date", ReleaseDateInfo:text r_date_info=>"r_date_info",
    Region:text region=>"region", RomInfo:text rominfo=>"rominfo",
    Section:text section=>"section", Tool:text tool=>"tool"
);

native_fields!(dump_serials_fields, NoIntroDatabaseDumpSerialsField, SourceSerials, 14;
    BoxBarcode:text box_barcode=>"box_barcode", BoxSerial:text box_serial=>"box_serial",
    ChipSerial:text chip_serial=>"chip_serial", DigitalSerial1:text digital_serial1=>"digital_serial1",
    DigitalSerial2:text digital_serial2=>"digital_serial2", LockoutSerial:text lockout_serial=>"lockout_serial",
    MediaSerial1:text media_serial1=>"media_serial1", MediaSerial2:text media_serial2=>"media_serial2",
    MediaSerial3:text media_serial3=>"media_serial3", MediaStamp:text mediastamp=>"mediastamp",
    PcbSerial:text pcb_serial=>"pcb_serial", RomChipSerial1:text romchip_serial1=>"romchip_serial1",
    RomChipSerial2:text romchip_serial2=>"romchip_serial2", SaveChipSerial:text savechip_serial=>"savechip_serial"
);

native_fields!(dump_file_fields, NoIntroDatabaseDumpFileField, SourceFile, 23;
    Bad:text bad=>"bad", Crc32:digest crc32=>"crc32", Date:text date=>"date",
    Extension:text extension=>"extension", Filter:text filter=>"filter",
    ForceName:text forcename=>"forcename", ForceSceneName:text forcescenename=>"forcescenename",
    Format:text format=>"format", Header:text header=>"header", Id:text id=>"id",
    Item:text item=>"item", Md5:digest md5=>"md5", Mia:text mia=>"mia",
    Note:text note=>"note", OriginSha256:digest origin_sha256=>"origin_sha256",
    OriginSize:text origin_size=>"origin_size", Serial:text serial=>"serial",
    Sha1:digest sha1=>"sha1", Sha256:digest sha256=>"sha256", Size:text size=>"source_size",
    Unique:text unique=>"unique", UpdateType:text update_type=>"update_type", Version:text version=>"version"
);

native_fields!(release_details_fields, NoIntroDatabaseReleaseDetailsField, ReleaseDetails, 17;
    ArchiveName:text archivename=>"archivename", Category:text category=>"category",
    Comment:text comment=>"comment", Date:text date=>"date", Directory:text dirname=>"dirname",
    Group:text group=>"group", Id:text id=>"id", NfoCrc32:digest nfo_crc32=>"nfo_crc32",
    NfoSize:text nfo_size=>"nfo_size", NfoCrc:digest nfocrc=>"nfocrc",
    NfoName:text nfoname=>"nfoname", NfoLegacySize:text nfosize=>"nfosize",
    Origin:text origin=>"origin", OriginalFormat:text originalformat=>"originalformat",
    Region:text region=>"region", RomInfo:text rominfo=>"rominfo", Tool:text tool=>"tool"
);

native_fields!(release_serials_fields, NoIntroDatabaseReleaseSerialsField, ReleaseSerials, 6;
    BoxBarcode:text box_barcode=>"box_barcode", BoxSerial:text box_serial=>"box_serial",
    MediaSerial1:text media_serial1=>"media_serial1", MediaStamp:text mediastamp=>"mediastamp",
    PcbSerial:text pcb_serial=>"pcb_serial", RomChipSerial1:text romchip_serial1=>"romchip_serial1"
);

native_fields!(release_file_fields, NoIntroDatabaseReleaseFileField, ReleaseFile, 17;
    Bad:text bad=>"bad", Crc32:digest crc32=>"crc32", Extension:text extension=>"extension",
    ForceName:text forcename=>"forcename", ForceSceneName:text forcescenename=>"forcescenename",
    Format:text format=>"format", Header:text header=>"header", Id:text id=>"id",
    Item:text item=>"item", Md5:digest md5=>"md5", Note:text note=>"note",
    Serial:text serial=>"serial", Sha1:digest sha1=>"sha1", Sha256:digest sha256=>"sha256",
    Size:text size=>"source_size", UpdateType:text update_type=>"update_type", Version:text version=>"version"
);

#[derive(Clone, Copy)]
enum NativeTable {
    Archive,
    DumpDetails,
    DumpSerials,
    DumpFile,
    ReleaseDetails,
    ReleaseSerials,
    ReleaseFile,
}

impl NativeTable {
    const fn table(self) -> &'static str {
        match self {
            Self::Archive => "no_intro_archive_descriptions",
            Self::DumpDetails => "no_intro_dump_details",
            Self::DumpSerials => "no_intro_dump_serials",
            Self::DumpFile => "no_intro_dump_files",
            Self::ReleaseDetails => "no_intro_release_details",
            Self::ReleaseSerials => "no_intro_release_serials",
            Self::ReleaseFile => "no_intro_release_files",
        }
    }

    const fn position_table(self) -> &'static str {
        match self {
            Self::Archive => "no_intro_archive_field_positions",
            Self::DumpDetails => "no_intro_dump_details_field_positions",
            Self::DumpSerials => "no_intro_dump_serials_field_positions",
            Self::DumpFile => "no_intro_dump_file_field_positions",
            Self::ReleaseDetails => "no_intro_release_details_field_positions",
            Self::ReleaseSerials => "no_intro_release_serials_field_positions",
            Self::ReleaseFile => "no_intro_release_file_field_positions",
        }
    }

    const fn owner_column(self) -> &'static str {
        match self {
            Self::Archive => "archive_id",
            Self::DumpDetails | Self::DumpSerials => "dump_source_id",
            Self::ReleaseDetails | Self::ReleaseSerials => "release_id",
            Self::DumpFile | Self::ReleaseFile => "occurrence_id",
        }
    }

    const fn keys(self) -> &'static str {
        match self {
            Self::Archive => "set_id,source_order,source_line,source_column",
            Self::DumpDetails => {
                "dump_source_id,source_order,source_line,source_column,opening_end_line,opening_end_column"
            }
            Self::DumpSerials => "dump_source_id,source_order,source_line,source_column",
            Self::ReleaseDetails => {
                "release_id,source_order,source_line,source_column,opening_end_line,opening_end_column"
            }
            Self::ReleaseSerials => "release_id,source_order,source_line,source_column",
            Self::DumpFile => {
                "occurrence_id,dump_source_id,set_id,source_order,source_line,source_column"
            }
            Self::ReleaseFile => {
                "occurrence_id,release_id,set_id,source_order,source_line,source_column"
            }
        }
    }
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn ordinal(value: usize) -> crate::Result<i64> {
    i64::try_from(value).map_err(|_| {
        crate::Error::DatabaseSchema("No-Intro source order exceeds SQLite range".into())
    })
}

fn insert_fields(
    conn: &mut SqliteConnection,
    table: NativeTable,
    keys: &[i64],
    fields: &[Field<'_>],
) -> crate::Result<i64> {
    let columns: Vec<_> = fields
        .iter()
        .filter_map(|field| match &field.value {
            FieldValue::Text(column, value) => Some((*column, *value)),
            FieldValue::Digest(_) | FieldValue::Clone(_) | FieldValue::Link(_) => None,
        })
        .collect();
    let mut sql = format!("INSERT INTO {} ({},", table.table(), table.keys());
    for (index, (column, _)) in columns.iter().enumerate() {
        if index != 0 {
            sql.push(',');
        }
        sql.push('"');
        sql.push_str(column);
        sql.push('"');
    }
    sql.push_str(") VALUES (");
    for index in 0..keys.len() + columns.len() {
        if index != 0 {
            sql.push(',');
        }
        sql.push('?');
    }
    sql.push_str(") RETURNING ");
    sql.push_str(table.owner_column());
    sql.push_str(" AS value");
    let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
    for key in keys {
        query = query.bind::<BigInt, _>(*key);
    }
    for (_, field) in columns {
        query = query.bind::<Nullable<Text>, _>(field.map(DeclaredText::as_str));
    }
    let id = query.get_result::<Id>(conn)?.value;
    let sql = format!(
        "INSERT INTO {} ({},field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)",
        table.position_table(),
        table.owner_column()
    );
    for field in fields {
        if let Some(declared) = field.value.declaration() {
            sql_query(&sql)
                .bind::<BigInt, _>(id)
                .bind::<BigInt, _>(field.kind)
                .bind::<BigInt, _>(ordinal(declared.source_order)?)
                .bind::<BigInt, _>(declared.location.line)
                .bind::<BigInt, _>(declared.location.column)
                .execute(conn)?;
        }
    }
    Ok(id)
}

#[derive(Clone, Copy)]
#[repr(usize)]
enum CountKind {
    Games,
    Archives,
    DumpSources,
    DumpDetails,
    DumpSerials,
    DumpFiles,
    Releases,
    ReleaseDetails,
    ReleaseSerials,
    ReleaseFiles,
    HeaderFields,
    ArchiveFields,
    DumpDetailsFields,
    DumpSerialsFields,
    DumpFileFields,
    ReleaseDetailsFields,
    ReleaseSerialsFields,
    ReleaseFileFields,
}

#[derive(Default)]
pub(super) struct ImportCounts([i64; 18]);

impl ImportCounts {
    pub(super) fn from_document(document: &NoIntroDatabaseDocument) -> crate::Result<Self> {
        let mut counts = Self::default();
        counts.add(
            CountKind::HeaderFields,
            document
                .header
                .as_ref()
                .map_or(0, |header| header.fields.len()),
        )?;
        Ok(counts)
    }

    fn add(&mut self, kind: CountKind, count: usize) -> crate::Result<()> {
        let value = &mut self.0[kind as usize];
        *value = value
            .checked_add(ordinal(count)?)
            .ok_or_else(|| crate::Error::DatabaseSchema("No-Intro parse count overflow".into()))?;
        Ok(())
    }

    fn fields(&mut self, kind: CountKind, fields: &[Field<'_>]) -> crate::Result<()> {
        self.add(
            kind,
            fields
                .iter()
                .filter(|field| field.value.declaration().is_some())
                .count(),
        )
    }

    pub(super) fn include_game(&mut self, game: &DatabaseGame) -> crate::Result<()> {
        self.add(CountKind::Games, 1)?;
        self.add(CountKind::Archives, game.archives.len())?;
        for archive in &game.archives {
            self.fields(CountKind::ArchiveFields, &archive_fields(archive))?;
        }
        for child in &game.source_or_release {
            match child {
                SourceOrRelease::Source(source) => {
                    self.add(CountKind::DumpSources, 1)?;
                    self.add(CountKind::DumpFiles, source.files.len())?;
                    if let Some(details) = &source.details {
                        self.add(CountKind::DumpDetails, 1)?;
                        self.fields(CountKind::DumpDetailsFields, &dump_details_fields(details))?;
                    }
                    if let Some(serials) = &source.serials {
                        self.add(CountKind::DumpSerials, 1)?;
                        self.fields(CountKind::DumpSerialsFields, &dump_serials_fields(serials))?;
                    }
                    for file in &source.files {
                        self.fields(CountKind::DumpFileFields, &dump_file_fields(file))?;
                    }
                }
                SourceOrRelease::Release(release) => {
                    self.add(CountKind::Releases, 1)?;
                    self.add(CountKind::ReleaseFiles, release.files.len())?;
                    if let Some(details) = &release.details {
                        self.add(CountKind::ReleaseDetails, 1)?;
                        self.fields(
                            CountKind::ReleaseDetailsFields,
                            &release_details_fields(details),
                        )?;
                    }
                    if let Some(serials) = &release.serials {
                        self.add(CountKind::ReleaseSerials, 1)?;
                        self.fields(
                            CountKind::ReleaseSerialsFields,
                            &release_serials_fields(serials),
                        )?;
                    }
                    for file in &release.files {
                        self.fields(CountKind::ReleaseFileFields, &release_file_fields(file))?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub(super) fn insert_document(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    document: &NoIntroDatabaseDocument,
) -> crate::Result<()> {
    sql_query("INSERT INTO no_intro_exports(snapshot_key,envelope_kind,header_present,source_line,source_column) VALUES(?,?,?,?,?)")
        .bind::<Text,_>(snapshot.as_str()).bind::<Text,_>(match document.envelope { EnvelopeKind::SingleDatafile=>"single_datafile", EnvelopeKind::SiblingHeaderDatafile=>"sibling_header_datafile" })
        .bind::<BigInt,_>(i64::from(document.header.is_some())).bind::<BigInt,_>(document.location.line).bind::<BigInt,_>(document.location.column).execute(conn)?;
    if let Some(header) = &document.header {
        sql_query("INSERT INTO no_intro_export_headers(snapshot_key,source_line,source_column) VALUES(?,?,?)")
            .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(header.location.line).bind::<BigInt,_>(header.location.column).execute(conn)?;
        for field in &header.fields {
            let kind = match field.kind {
                HeaderFieldKind::Author => 0,
                HeaderFieldKind::Piracy => 1,
                HeaderFieldKind::Trademarks => 2,
                HeaderFieldKind::Url => 3,
                HeaderFieldKind::Version => 4,
            };
            sql_query("INSERT INTO no_intro_header_fields(snapshot_key,source_order,field_kind,value,source_line,source_column) VALUES(?,?,?,?,?,?)")
                .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(ordinal(field.value.source_order)?)
                .bind::<BigInt,_>(kind).bind::<Text,_>(field.value.as_str()).bind::<BigInt,_>(field.value.location.line).bind::<BigInt,_>(field.value.location.column).execute(conn)?;
        }
    }
    sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES(?,'root',0)")
        .bind::<Text, _>(snapshot.as_str())
        .execute(conn)?;
    Ok(())
}

pub(super) fn seal_document(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    counts: &ImportCounts,
) -> crate::Result<()> {
    let mut query = sql_query("INSERT INTO no_intro_database_parse_counts(snapshot_key,game_count,archive_count,dump_source_count,dump_details_count,dump_serials_count,dump_file_count,release_count,release_details_count,release_serials_count,release_file_count,header_field_count,archive_field_count,dump_details_field_count,dump_serials_field_count,dump_file_field_count,release_details_field_count,release_serials_field_count,release_file_field_count) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)").into_boxed::<diesel::sqlite::Sqlite>().bind::<Text,_>(snapshot.as_str());
    for value in counts.0 {
        query = query.bind::<BigInt, _>(value);
    }
    query.execute(conn)?;
    Ok(())
}

pub(super) fn insert_game(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    game: &DatabaseGame,
) -> crate::Result<()> {
    let set = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT set_group_id,'no_intro_database_game',?,?,?,? FROM catalog_set_groups WHERE snapshot_key=? AND kind='root' RETURNING set_id AS value")
        .bind::<BigInt,_>(ordinal(game.list_order)?).bind::<Text,_>(game.name.as_str())
        .bind::<BigInt,_>(game.location.line).bind::<BigInt,_>(game.location.column).bind::<Text,_>(snapshot.as_str()).get_result::<Id>(conn)?.value;
    let set = CatalogSetId::from_database(set);
    sql_query("INSERT INTO no_intro_database_games(set_id,name_source_order,name_source_line,name_source_column) VALUES(?,?,?,?)")
        .bind::<BigInt,_>(set.as_i64()).bind::<BigInt,_>(ordinal(game.name.source_order)?)
        .bind::<BigInt,_>(game.name.location.line).bind::<BigInt,_>(game.name.location.column).execute(conn)?;
    for archive in &game.archives {
        let id = NoIntroArchiveId::try_from(insert_fields(
            conn,
            NativeTable::Archive,
            &[
                set.as_i64(),
                ordinal(archive.source_order)?,
                archive.location.line,
                archive.location.column,
            ],
            &archive_fields(archive),
        )?)?;
        insert_archive_links(conn, snapshot, id, archive)?;
    }
    let mut file_order = 0;
    for child in &game.source_or_release {
        match child {
            SourceOrRelease::Source(source) => {
                insert_dump_source(conn, set, source, &mut file_order)?;
            }
            SourceOrRelease::Release(release) => {
                insert_release(conn, set, release, &mut file_order)?;
            }
        }
    }
    Ok(())
}

fn insert_dump_source(
    conn: &mut SqliteConnection,
    set: CatalogSetId,
    source: &DumpSource,
    file_order: &mut usize,
) -> crate::Result<()> {
    let id = DumpSourceId(sql_query("INSERT INTO no_intro_dump_sources(set_id,source_order,source_line,source_column) VALUES(?,?,?,?) RETURNING dump_source_id AS value")
        .bind::<BigInt,_>(set.as_i64()).bind::<BigInt,_>(ordinal(source.source_order)?)
        .bind::<BigInt,_>(source.location.line).bind::<BigInt,_>(source.location.column).get_result::<Id>(conn)?.value);
    if let Some(details) = &source.details {
        insert_fields(
            conn,
            NativeTable::DumpDetails,
            &[
                id.0,
                ordinal(details.source_order)?,
                details.location.line,
                details.location.column,
                details.opening_end.line,
                details.opening_end.column,
            ],
            &dump_details_fields(details),
        )?;
    }
    if let Some(serials) = &source.serials {
        insert_fields(
            conn,
            NativeTable::DumpSerials,
            &[
                id.0,
                ordinal(serials.source_order)?,
                serials.location.line,
                serials.location.column,
            ],
            &dump_serials_fields(serials),
        )?;
    }
    for file in &source.files {
        let occurrence =
            insert_occurrence(conn, set, *file_order, "no_intro_database_source_file")?;
        insert_fields(
            conn,
            NativeTable::DumpFile,
            &[
                occurrence.database_value(),
                id.0,
                set.as_i64(),
                ordinal(file.source_order)?,
                file.location.line,
                file.location.column,
            ],
            &dump_file_fields(file),
        )?;
        insert_file_hashes(
            conn,
            FileTable::Dump,
            occurrence,
            [
                file.crc32.as_ref(),
                file.md5.as_ref(),
                file.sha1.as_ref(),
                file.sha256.as_ref(),
            ],
        )?;
        if let Some(origin) = &file.origin_sha256 {
            let hash = insert_digest(conn, "sha256", origin)?;
            insert_file_digest_row(conn, FileTable::Dump, occurrence, 4, hash, origin)?;
            record_occurrence_digest_assertions(
                conn,
                occurrence,
                ContentDigestAssertions::new(
                    "source_origin",
                    None,
                    None,
                    None,
                    origin.value.as_deref(),
                ),
                "source_declared",
            )?;
        }
        *file_order += 1;
    }
    Ok(())
}

fn insert_release(
    conn: &mut SqliteConnection,
    set: CatalogSetId,
    release: &DatabaseRelease,
    file_order: &mut usize,
) -> crate::Result<()> {
    let id = ReleaseId(sql_query("INSERT INTO no_intro_releases(set_id,source_order,source_line,source_column) VALUES(?,?,?,?) RETURNING release_id AS value")
        .bind::<BigInt,_>(set.as_i64()).bind::<BigInt,_>(ordinal(release.source_order)?)
        .bind::<BigInt,_>(release.location.line).bind::<BigInt,_>(release.location.column).get_result::<Id>(conn)?.value);
    if let Some(details) = &release.details {
        insert_fields(
            conn,
            NativeTable::ReleaseDetails,
            &[
                id.0,
                ordinal(details.source_order)?,
                details.location.line,
                details.location.column,
                details.opening_end.line,
                details.opening_end.column,
            ],
            &release_details_fields(details),
        )?;
        for (name, digest) in [
            ("nfo_crc32", details.nfo_crc32.as_ref()),
            ("nfocrc", details.nfocrc.as_ref()),
        ] {
            if let Some(digest) = digest {
                let hash = insert_digest(conn, "crc32", digest)?;
                sql_query("INSERT INTO no_intro_release_nfo_hashes(release_id,source_hash_field,hash_id,presence,scope,invalid_literal) VALUES(?,?,?,'present','nfo_companion',?)")
                    .bind::<BigInt,_>(id.0).bind::<Text,_>(name).bind::<Nullable<BigInt>,_>(hash)
                    .bind::<Nullable<Text>,_>(hash.is_none().then_some(digest.source.as_str())).execute(conn)?;
            }
        }
    }
    if let Some(serials) = &release.serials {
        insert_fields(
            conn,
            NativeTable::ReleaseSerials,
            &[
                id.0,
                ordinal(serials.source_order)?,
                serials.location.line,
                serials.location.column,
            ],
            &release_serials_fields(serials),
        )?;
    }
    for file in &release.files {
        let occurrence =
            insert_occurrence(conn, set, *file_order, "no_intro_database_release_file")?;
        insert_fields(
            conn,
            NativeTable::ReleaseFile,
            &[
                occurrence.database_value(),
                id.0,
                set.as_i64(),
                ordinal(file.source_order)?,
                file.location.line,
                file.location.column,
            ],
            &release_file_fields(file),
        )?;
        insert_file_hashes(
            conn,
            FileTable::Release,
            occurrence,
            [
                file.crc32.as_ref(),
                file.md5.as_ref(),
                file.sha1.as_ref(),
                file.sha256.as_ref(),
            ],
        )?;
        *file_order += 1;
    }
    Ok(())
}

fn insert_occurrence(
    conn: &mut SqliteConnection,
    set: CatalogSetId,
    order: usize,
    kind: &str,
) -> crate::Result<OccurrenceId> {
    let id=sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind) VALUES(?,?,?) RETURNING occurrence_id AS value")
        .bind::<BigInt,_>(set.as_i64()).bind::<BigInt,_>(ordinal(order)?).bind::<Text,_>(kind).get_result::<Id>(conn)?.value;
    Ok(OccurrenceId::from_database(id))
}

fn insert_digest(
    conn: &mut SqliteConnection,
    algorithm: &str,
    digest: &DatabaseDigest,
) -> crate::Result<Option<i64>> {
    let Some(bytes) = digest.value.as_deref() else {
        return Ok(None);
    };
    Ok(Some(crate::storage::catalog_content::intern_digest(
        conn, algorithm, bytes,
    )?))
}

fn insert_file_hashes(
    conn: &mut SqliteConnection,
    table: FileTable,
    occurrence: OccurrenceId,
    digests: [Option<&DatabaseDigest>; 4],
) -> crate::Result<()> {
    for (kind, (algorithm, digest)) in ["crc32", "md5", "sha1", "sha256"]
        .into_iter()
        .zip(digests)
        .enumerate()
    {
        if let Some(digest) = digest {
            let hash = insert_digest(conn, algorithm, digest)?;
            insert_file_digest_row(conn, table, occurrence, ordinal(kind)?, hash, digest)?;
        }
    }
    let [crc, md5, sha1, sha256] =
        digests.map(|digest| digest.and_then(|digest| digest.value.as_deref()));
    // Export metadata has not established a universal whole-file hash contract.
    // Unknown scope is retained, not upgraded from a format or filename guess.
    record_occurrence_digest_assertions(
        conn,
        occurrence,
        ContentDigestAssertions::new("unknown", crc, md5, sha1, sha256),
        "source_declared",
    )
}

fn insert_file_digest_row(
    conn: &mut SqliteConnection,
    table: FileTable,
    occurrence: OccurrenceId,
    kind: i64,
    hash: Option<i64>,
    digest: &DatabaseDigest,
) -> crate::Result<()> {
    let table = table.digest_table();
    sql_query(format!(
        "INSERT INTO {table}(occurrence_id,field_kind,digest_id,invalid_literal) VALUES(?,?,?,?)"
    ))
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(kind)
    .bind::<Nullable<BigInt>, _>(hash)
    .bind::<Nullable<Text>, _>(hash.is_none().then_some(digest.source.as_str()))
    .execute(conn)?;
    Ok(())
}

fn insert_archive_links(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    id: NoIntroArchiveId,
    archive: &ArchiveDescription,
) -> crate::Result<()> {
    if let Some(clone) = &archive.clone {
        match clone {
            ArchiveClone::ParentMarker(_) => {
                sql_query(
                    "INSERT INTO no_intro_archive_clone_markers(archive_id,marker) VALUES(?,'P')",
                )
                .bind::<BigInt, _>(id.as_i64())
                .execute(conn)?;
            }
            ArchiveClone::OtherValue(text) => insert_reference(
                conn,
                snapshot,
                ReferenceOwner::NoIntroArchive {
                    archive: id,
                    field: NoIntroArchiveReferenceField::Clone,
                },
                text.as_str(),
            )?,
        }
    }
    if let Some(text) = &archive.mergeof {
        insert_reference(
            conn,
            snapshot,
            ReferenceOwner::NoIntroArchive {
                archive: id,
                field: NoIntroArchiveReferenceField::MergeOf,
            },
            text.as_str(),
        )?;
    }
    Ok(())
}
