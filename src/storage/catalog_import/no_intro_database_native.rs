use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
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
     $($variant:ident = $code:literal: $storage:ident $field:ident => $column:literal),+ $(,)?) => {
        #[derive(Clone, Copy)]
        #[repr(i64)]
        enum $enum { $($variant = $code),+ }

        const fn $function(owner: &$model) -> [Field<'_>; $count] {
            [$(Field {
                kind: $enum::$variant as i64,
                value: field_value!(owner, $storage, $field, $column),
            }),+]
        }
    };
}

native_fields!(archive_fields, ArchiveField, ArchiveDescription, 32;
    Additional=0:text additional=>"additional", Adult=1:text adult=>"adult",
    Aftermarket=2:text aftermarket=>"aftermarket", Alt=3:text alt=>"alt", Bios=4:text bios=>"bios",
    Categories=5:text categories=>"categories", Complete=6:text complete=>"complete",
    Dat=7:text dat=>"dat", DatterNote=8:text datter_note=>"datter_note",
    Description=9:text description=>"description", Devstatus=10:text devstatus=>"devstatus",
    GameId1=11:text gameid1=>"gameid1", GameId2=12:text gameid2=>"gameid2",
    Langchecked=13:text langchecked=>"langchecked", Languages=14:text languages=>"languages",
    Licensed=15:text licensed=>"licensed", Listed=16:text listed=>"listed",
    Mergename=17:text mergename=>"mergename", Name=18:text name=>"name",
    NameAlt=19:text name_alt=>"name_alt", Number=20:text number=>"number",
    Physical=21:text physical=>"physical", Region=22:text region=>"region",
    Regparent=23:text regparent=>"regparent", Showlang=24:text showlang=>"showlang",
    Special1=25:text special1=>"special1", Special2=26:text special2=>"special2",
    StickyNote=27:text sticky_note=>"sticky_note", Version1=28:text version1=>"version1",
    Version2=29:text version2=>"version2", Clone=30:clone clone=>"clone",
    MergeOf=31:link mergeof=>"mergeof"
);

native_fields!(dump_details_fields, DumpDetailsField, SourceDetails, 20;
    Comment1=0:text comment1=>"comment1", Comment2=1:text comment2=>"comment2",
    DumpDate=2:text d_date=>"d_date", DumpDateInfo=3:text d_date_info=>"d_date_info",
    Dumper=4:text dumper=>"dumper", Id=5:text id=>"id", Link1=6:text link1=>"link1",
    Link2=7:text link2=>"link2", Link3=8:text link3=>"link3", MediaTitle=9:text media_title=>"media_title",
    NoDump=10:text nodump=>"nodump", Origin=11:text origin=>"origin",
    OriginalFormat=12:text originalformat=>"originalformat", Project=13:text project=>"project",
    ReleaseDate=14:text r_date=>"r_date", ReleaseDateInfo=15:text r_date_info=>"r_date_info",
    Region=16:text region=>"region", RomInfo=17:text rominfo=>"rominfo",
    Section=18:text section=>"section", Tool=19:text tool=>"tool"
);

native_fields!(dump_serials_fields, DumpSerialsField, SourceSerials, 14;
    BoxBarcode=0:text box_barcode=>"box_barcode", BoxSerial=1:text box_serial=>"box_serial",
    ChipSerial=2:text chip_serial=>"chip_serial", DigitalSerial1=3:text digital_serial1=>"digital_serial1",
    DigitalSerial2=4:text digital_serial2=>"digital_serial2", LockoutSerial=5:text lockout_serial=>"lockout_serial",
    MediaSerial1=6:text media_serial1=>"media_serial1", MediaSerial2=7:text media_serial2=>"media_serial2",
    MediaSerial3=8:text media_serial3=>"media_serial3", MediaStamp=9:text mediastamp=>"mediastamp",
    PcbSerial=10:text pcb_serial=>"pcb_serial", RomChipSerial1=11:text romchip_serial1=>"romchip_serial1",
    RomChipSerial2=12:text romchip_serial2=>"romchip_serial2", SaveChipSerial=13:text savechip_serial=>"savechip_serial"
);

native_fields!(dump_file_fields, DumpFileField, SourceFile, 23;
    Bad=0:text bad=>"bad", Crc32=1:digest crc32=>"crc32", Date=2:text date=>"date",
    Extension=3:text extension=>"extension", Filter=4:text filter=>"filter",
    ForceName=5:text forcename=>"forcename", ForceSceneName=6:text forcescenename=>"forcescenename",
    Format=7:text format=>"format", Header=8:text header=>"header", Id=9:text id=>"id",
    Item=10:text item=>"item", Md5=11:digest md5=>"md5", Mia=12:text mia=>"mia",
    Note=13:text note=>"note", OriginSha256=14:digest origin_sha256=>"origin_sha256",
    OriginSize=15:text origin_size=>"origin_size", Serial=16:text serial=>"serial",
    Sha1=17:digest sha1=>"sha1", Sha256=18:digest sha256=>"sha256", Size=19:text size=>"source_size",
    Unique=20:text unique=>"unique", UpdateType=21:text update_type=>"update_type", Version=22:text version=>"version"
);

native_fields!(release_details_fields, ReleaseDetailsField, ReleaseDetails, 17;
    ArchiveName=0:text archivename=>"archivename", Category=1:text category=>"category",
    Comment=2:text comment=>"comment", Date=3:text date=>"date", Directory=4:text dirname=>"dirname",
    Group=5:text group=>"group", Id=6:text id=>"id", NfoCrc32=7:digest nfo_crc32=>"nfo_crc32",
    NfoSize=8:text nfo_size=>"nfo_size", NfoCrc=9:digest nfocrc=>"nfocrc",
    NfoName=10:text nfoname=>"nfoname", NfoLegacySize=11:text nfosize=>"nfosize",
    Origin=12:text origin=>"origin", OriginalFormat=13:text originalformat=>"originalformat",
    Region=14:text region=>"region", RomInfo=15:text rominfo=>"rominfo", Tool=16:text tool=>"tool"
);

native_fields!(release_serials_fields, ReleaseSerialsField, ReleaseSerials, 6;
    BoxBarcode=0:text box_barcode=>"box_barcode", BoxSerial=1:text box_serial=>"box_serial",
    MediaSerial1=2:text media_serial1=>"media_serial1", MediaStamp=3:text mediastamp=>"mediastamp",
    PcbSerial=4:text pcb_serial=>"pcb_serial", RomChipSerial1=5:text romchip_serial1=>"romchip_serial1"
);

native_fields!(release_file_fields, ReleaseFileField, ReleaseFile, 17;
    Bad=0:text bad=>"bad", Crc32=1:digest crc32=>"crc32", Extension=2:text extension=>"extension",
    ForceName=3:text forcename=>"forcename", ForceSceneName=4:text forcescenename=>"forcescenename",
    Format=5:text format=>"format", Header=6:text header=>"header", Id=7:text id=>"id",
    Item=8:text item=>"item", Md5=9:digest md5=>"md5", Note=10:text note=>"note",
    Serial=11:text serial=>"serial", Sha1=12:digest sha1=>"sha1", Sha256=13:digest sha256=>"sha256",
    Size=14:text size=>"source_size", UpdateType=15:text update_type=>"update_type", Version=16:text version=>"version"
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
    sql_query("INSERT OR IGNORE INTO digest_values(algorithm,digest) VALUES(?,?)")
        .bind::<Text, _>(algorithm)
        .bind::<Binary, _>(bytes)
        .execute(conn)?;
    Ok(Some(
        sql_query("SELECT digest_id AS value FROM digest_values WHERE algorithm=? AND digest=?")
            .bind::<Text, _>(algorithm)
            .bind::<Binary, _>(bytes)
            .get_result::<Id>(conn)?
            .value,
    ))
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
