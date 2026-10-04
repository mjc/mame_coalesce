use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    domain::{
        CatalogSetId, DocumentKey, ImportRunKey, NoIntroArchiveId, NoIntroArchiveReferenceField,
        SnapshotKey,
    },
    logiqx::RecordLocation,
    no_intro_db_xml::{
        ArchiveClone, ArchiveDescription, DatabaseDigest, DatabaseGame, EnvelopeKind,
        HeaderFieldKind, NoIntroDatabaseDocument, RecoveryCursor, ReleaseDetails, ReleaseFile,
        ReleaseSerials, SourceDetails, SourceFile, SourceOrRelease, SourceSerials, XmlSourceExtent,
    },
    storage::{
        catalog_content::{ContentDigestAssertions, record_occurrence_digest_assertions},
        catalog_files::{NoIntroDumpSourceId, NoIntroReleaseId},
        catalog_identity::OccurrenceId,
        import_diagnostics::{self, DiagnosticOrder, NoIntroDiagnosticOwner},
        no_intro_database_fields::{
            NoIntroDatabaseArchiveField, NoIntroDatabaseDumpDetailsField,
            NoIntroDatabaseDumpFileField, NoIntroDatabaseDumpSerialsField,
            NoIntroDatabaseReleaseDetailsField, NoIntroDatabaseReleaseFileField,
            NoIntroDatabaseReleaseSerialsField,
        },
    },
    xml_reader::DeclaredText,
};

use super::{
    SnapshotPublication,
    reported_relationships::{ReferenceOwner, insert_reference},
};

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
            Self::Archive => {
                "set_id,source_order,source_line,source_column,source_end_line,source_end_column"
            }
            Self::DumpDetails => {
                "dump_source_id,source_order,source_line,source_column,source_end_line,source_end_column,opening_end_line,opening_end_column"
            }
            Self::DumpSerials => {
                "dump_source_id,source_order,source_line,source_column,source_end_line,source_end_column"
            }
            Self::ReleaseDetails => {
                "release_id,source_order,source_line,source_column,source_end_line,source_end_column,opening_end_line,opening_end_column"
            }
            Self::ReleaseSerials => {
                "release_id,source_order,source_line,source_column,source_end_line,source_end_column"
            }
            Self::DumpFile => {
                "occurrence_id,dump_source_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column"
            }
            Self::ReleaseFile => {
                "occurrence_id,release_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column"
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
    sql_query("INSERT INTO no_intro_exports(snapshot_key,envelope_kind,header_present,source_line,source_column,document_end_line,document_end_column) VALUES(?,?,?,?,?,?,?)")
        .bind::<Text,_>(snapshot.as_str()).bind::<Text,_>(match document.envelope { EnvelopeKind::SingleDatafile=>"single_datafile", EnvelopeKind::SiblingHeaderDatafile=>"sibling_header_datafile" })
        .bind::<BigInt,_>(i64::from(document.header.is_some())).bind::<BigInt,_>(document.location.line).bind::<BigInt,_>(document.location.column)
        .bind::<BigInt,_>(document.extent.end().line).bind::<BigInt,_>(document.extent.end().column).execute(conn)?;
    if let Some(header) = &document.header {
        sql_query("INSERT INTO no_intro_export_headers(snapshot_key,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,?,?,?)")
            .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(header.extent.start().line).bind::<BigInt,_>(header.extent.start().column)
            .bind::<BigInt,_>(header.extent.end().line).bind::<BigInt,_>(header.extent.end().column).execute(conn)?;
        for field in &header.fields {
            let kind = match field.kind {
                HeaderFieldKind::Author => 0,
                HeaderFieldKind::Piracy => 1,
                HeaderFieldKind::Trademarks => 2,
                HeaderFieldKind::Url => 3,
                HeaderFieldKind::Version => 4,
            };
            sql_query("INSERT INTO no_intro_header_fields(snapshot_key,source_order,field_kind,value,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,?,?,?,?,?,?)")
                .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(ordinal(field.source_order)?)
                .bind::<BigInt,_>(kind).bind::<Text,_>(field.value.as_str()).bind::<BigInt,_>(field.extent.start().line).bind::<BigInt,_>(field.extent.start().column)
                .bind::<BigInt,_>(field.extent.end().line).bind::<BigInt,_>(field.extent.end().column).execute(conn)?;
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

/// Import-local SQL state; never borrows the decoded XML or owns warning events.
pub(super) struct WarningWriter<'a> {
    pub(super) conn: &'a mut SqliteConnection,
    pub(super) publication: &'a SnapshotPublication,
    pub(super) run: &'a ImportRunKey,
    pub(super) document: &'a DocumentKey,
    pub(super) count: &'a mut usize,
}

impl WarningWriter<'_> {
    const fn pending(&self) -> bool {
        matches!(self.publication, SnapshotPublication::Pending(_))
    }

    pub(super) fn emit_before(
        &mut self,
        warnings: &mut RecoveryCursor<'_>,
        boundary: crate::logiqx::RecordLocation,
        owner: &NoIntroDiagnosticOwner,
    ) -> crate::Result<()> {
        let extent = owner.link().extent;
        while warnings.peek().is_some_and(|warning| {
            (warning.location.line, warning.location.column) < (boundary.line, boundary.column)
        }) {
            let warning = warnings
                .next()
                .ok_or_else(|| invalid_native("warning cursor lost its peek"))?;
            if !extent.contains(warning.location) {
                return Err(invalid_native(
                    "warning lies outside its actual catalog owner",
                ));
            }
            let order = DiagnosticOrder::new(ordinal(*self.count)?)
                .ok_or_else(|| invalid_native("negative diagnostic order"))?;
            let key = import_diagnostics::insert_recovery_warning(
                self.conn,
                self.run,
                self.document,
                order,
                &warning,
            )?;
            import_diagnostics::link_no_intro_owner(
                self.conn,
                &key,
                self.run,
                self.publication.key(),
                owner,
            )?;
            *self.count = self
                .count
                .checked_add(1)
                .ok_or_else(|| invalid_native("diagnostic count overflow"))?;
        }
        Ok(())
    }

    fn row(
        &mut self,
        table: NativeTable,
        parent: (&str, i64),
        source_order: usize,
        extent: XmlSourceExtent,
        keys: &[i64],
        fields: &[Field<'_>],
    ) -> crate::Result<i64> {
        if self.pending() {
            return insert_fields(self.conn, table, keys, fields);
        }
        self.seek_row(
            table.table(),
            table.owner_column(),
            parent,
            source_order,
            extent,
        )
    }

    fn seek_row(
        &mut self,
        table: &str,
        id_column: &str,
        parent: (&str, i64),
        source_order: usize,
        extent: XmlSourceExtent,
    ) -> crate::Result<i64> {
        let (parent_column, parent_id) = parent;
        let sql = format!(
            "SELECT {id_column} AS value FROM {table} WHERE {parent_column}=? AND source_order=? AND typeof({id_column})='integer' AND {id_column}>0 AND typeof({parent_column})='integer' AND typeof(source_order)='integer' AND typeof(source_line)='integer' AND typeof(source_column)='integer' AND typeof(source_end_line)='integer' AND typeof(source_end_column)='integer' AND source_line=? AND source_column=? AND source_end_line=? AND source_end_column=?"
        );
        Ok(sql_query(sql)
            .bind::<BigInt, _>(parent_id)
            .bind::<BigInt, _>(ordinal(source_order)?)
            .bind::<BigInt, _>(extent.start().line)
            .bind::<BigInt, _>(extent.start().column)
            .bind::<BigInt, _>(extent.end().line)
            .bind::<BigInt, _>(extent.end().column)
            .get_result::<Id>(self.conn)?
            .value)
    }

    pub(super) fn document(
        &mut self,
        document: &NoIntroDatabaseDocument,
        warnings: &mut RecoveryCursor<'_>,
    ) -> crate::Result<()> {
        let snapshot = self.publication.key().clone();
        if self.pending() {
            insert_document(self.conn, &snapshot, document)?;
        } else {
            sql_query("SELECT 1 AS value FROM no_intro_exports WHERE snapshot_key=? AND typeof(snapshot_key)='text' AND typeof(source_line)='integer' AND typeof(source_column)='integer' AND typeof(document_end_line)='integer' AND typeof(document_end_column)='integer' AND typeof(header_present)='integer' AND source_line=? AND source_column=? AND document_end_line=? AND document_end_column=? AND header_present=? AND envelope_kind=?")
                .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(document.location.line).bind::<BigInt,_>(document.location.column)
                .bind::<BigInt,_>(document.extent.end().line).bind::<BigInt,_>(document.extent.end().column)
                .bind::<BigInt,_>(i64::from(document.header.is_some()))
                .bind::<Text,_>(match document.envelope { EnvelopeKind::SingleDatafile=>"single_datafile", EnvelopeKind::SiblingHeaderDatafile=>"sibling_header_datafile" })
                .get_result::<Id>(self.conn)?;
        }
        let export = NoIntroDiagnosticOwner::ExportDocument {
            snapshot: snapshot.clone(),
            extent: document.extent,
        };
        if let Some(header) = &document.header {
            let owner = NoIntroDiagnosticOwner::ExportHeader {
                snapshot: snapshot.clone(),
                extent: header.extent,
            };
            if !self.pending() {
                self.validate_snapshot_extent(
                    "no_intro_export_headers",
                    &snapshot,
                    None,
                    header.extent,
                )?;
            }
            self.emit_before(warnings, header.extent.start(), &export)?;
            for field in &header.fields {
                if !self.pending() {
                    self.validate_snapshot_extent(
                        "no_intro_header_fields",
                        &snapshot,
                        Some(ordinal(field.source_order)?),
                        field.extent,
                    )?;
                }
                self.emit_before(warnings, field.extent.start(), &owner)?;
                let field_owner = NoIntroDiagnosticOwner::HeaderField {
                    snapshot: snapshot.clone(),
                    source_order: ordinal(field.source_order)?,
                    extent: field.extent,
                };
                self.emit_before(warnings, field.extent.end(), &field_owner)?;
            }
            self.emit_before(warnings, header.extent.end(), &owner)?;
        }
        Ok(())
    }

    fn validate_snapshot_extent(
        &mut self,
        table: &str,
        snapshot: &SnapshotKey,
        order: Option<i64>,
        extent: XmlSourceExtent,
    ) -> crate::Result<()> {
        let order_clause = if order.is_some() {
            " AND typeof(source_order)='integer' AND source_order=?"
        } else {
            ""
        };
        let mut query = sql_query(format!("SELECT 1 AS value FROM {table} WHERE typeof(snapshot_key)='text' AND snapshot_key=? AND typeof(source_line)='integer' AND typeof(source_column)='integer' AND typeof(source_end_line)='integer' AND typeof(source_end_column)='integer' AND source_line=? AND source_column=? AND source_end_line=? AND source_end_column=?{order_clause}"))
            .into_boxed::<diesel::sqlite::Sqlite>().bind::<Text,_>(snapshot.as_str())
            .bind::<BigInt,_>(extent.start().line).bind::<BigInt,_>(extent.start().column)
            .bind::<BigInt,_>(extent.end().line).bind::<BigInt,_>(extent.end().column);
        if let Some(order) = order {
            query = query.bind::<BigInt, _>(order);
        }
        query.get_result::<Id>(self.conn)?;
        Ok(())
    }

    pub(super) fn game(
        &mut self,
        game: &DatabaseGame,
        envelope: XmlSourceExtent,
        warnings: &mut RecoveryCursor<'_>,
    ) -> crate::Result<()> {
        let snapshot = self.publication.key().clone();
        let export = NoIntroDiagnosticOwner::ExportDocument {
            snapshot: snapshot.clone(),
            extent: envelope,
        };
        self.emit_before(warnings, game.extent.start(), &export)?;
        let set = if self.pending() {
            let set = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT set_group_id,'no_intro_database_game',?,?,?,? FROM catalog_set_groups WHERE snapshot_key=? AND kind='root' RETURNING set_id AS value")
                .bind::<BigInt,_>(ordinal(game.list_order)?).bind::<Text,_>(game.name.as_str())
                .bind::<BigInt,_>(game.extent.start().line).bind::<BigInt,_>(game.extent.start().column)
                .bind::<Text,_>(snapshot.as_str()).get_result::<Id>(self.conn)?.value;
            sql_query("INSERT INTO no_intro_database_games(set_id,name_source_order,name_source_line,name_source_column,source_end_line,source_end_column) VALUES(?,?,?,?,?,?)")
                .bind::<BigInt,_>(set).bind::<BigInt,_>(ordinal(game.name.source_order)?)
                .bind::<BigInt,_>(game.name.location.line).bind::<BigInt,_>(game.name.location.column)
                .bind::<BigInt,_>(game.extent.end().line).bind::<BigInt,_>(game.extent.end().column).execute(self.conn)?;
            set
        } else {
            sql_query("SELECT s.set_id AS value FROM catalog_set_groups g JOIN catalog_sets s USING(set_group_id) JOIN no_intro_database_games n USING(set_id) WHERE g.snapshot_key=? AND g.kind='root' AND s.source_element_kind='no_intro_database_game' AND typeof(s.set_id)='integer' AND s.set_id>0 AND typeof(s.list_order)='integer' AND s.list_order=? AND typeof(s.source_line)='integer' AND typeof(s.source_column)='integer' AND typeof(n.source_end_line)='integer' AND typeof(n.source_end_column)='integer' AND s.source_line=? AND s.source_column=? AND n.source_end_line=? AND n.source_end_column=?")
                .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(ordinal(game.list_order)?)
                .bind::<BigInt,_>(game.extent.start().line).bind::<BigInt,_>(game.extent.start().column)
                .bind::<BigInt,_>(game.extent.end().line).bind::<BigInt,_>(game.extent.end().column).get_result::<Id>(self.conn)?.value
        };
        let set = CatalogSetId::from_database(set);
        let owner = NoIntroDiagnosticOwner::Game {
            id: set,
            extent: game.extent,
        };
        let mut archives = game.archives.iter().peekable();
        let mut histories = game.source_or_release.iter().peekable();
        let mut file_order = 0;
        while archives.peek().is_some() || histories.peek().is_some() {
            if archives.peek().is_some_and(|archive| {
                histories
                    .peek()
                    .is_none_or(|history| archive.source_order < history.source_order())
            }) {
                let archive = archives
                    .next()
                    .ok_or_else(|| invalid_native("archive iterator lost its peek"))?;
                self.emit_before(warnings, archive.extent.start(), &owner)?;
                let id = NoIntroArchiveId::try_from(self.row(
                    NativeTable::Archive,
                    ("set_id", set.as_i64()),
                    archive.source_order,
                    archive.extent,
                    &[
                        set.as_i64(),
                        ordinal(archive.source_order)?,
                        archive.extent.start().line,
                        archive.extent.start().column,
                        archive.extent.end().line,
                        archive.extent.end().column,
                    ],
                    &archive_fields(archive),
                )?)?;
                if self.pending() {
                    insert_archive_links(self.conn, &snapshot, id, archive)?;
                }
                self.emit_before(
                    warnings,
                    archive.extent.end(),
                    &NoIntroDiagnosticOwner::ArchiveDescription {
                        id,
                        extent: archive.extent,
                    },
                )?;
            } else {
                let history = histories
                    .next()
                    .ok_or_else(|| invalid_native("history iterator lost its peek"))?;
                self.history(set, history, &owner, &mut file_order, warnings)?;
            }
        }
        self.emit_before(warnings, game.extent.end(), &owner)
    }

    fn history(
        &mut self,
        set: CatalogSetId,
        history: &SourceOrRelease,
        game: &NoIntroDiagnosticOwner,
        file_order: &mut usize,
        warnings: &mut RecoveryCursor<'_>,
    ) -> crate::Result<()> {
        match history {
            SourceOrRelease::Source(source) => {
                self.emit_before(warnings, source.extent.start(), game)?;
                let id = NoIntroDumpSourceId::try_from(self.history_id(
                    "no_intro_dump_sources",
                    "dump_source_id",
                    set,
                    source.source_order,
                    source.extent,
                )?)?;
                let owner = NoIntroDiagnosticOwner::DumpSource {
                    id,
                    extent: source.extent,
                };
                let children = ordered_history_children(
                    source.details.as_ref().map(HistoryChild::DumpDetails),
                    source.serials.as_ref().map(HistoryChild::DumpSerials),
                    source.files.iter().map(HistoryChild::DumpFile),
                );
                self.history_children(set, id.as_i64(), &owner, children, file_order, warnings)?;
                self.emit_before(warnings, source.extent.end(), &owner)
            }
            SourceOrRelease::Release(release) => {
                self.emit_before(warnings, release.extent.start(), game)?;
                let id = NoIntroReleaseId::try_from(self.history_id(
                    "no_intro_releases",
                    "release_id",
                    set,
                    release.source_order,
                    release.extent,
                )?)?;
                let owner = NoIntroDiagnosticOwner::Release {
                    id,
                    extent: release.extent,
                };
                let children = ordered_history_children(
                    release.details.as_ref().map(HistoryChild::ReleaseDetails),
                    release.serials.as_ref().map(HistoryChild::ReleaseSerials),
                    release.files.iter().map(HistoryChild::ReleaseFile),
                );
                self.history_children(set, id.as_i64(), &owner, children, file_order, warnings)?;
                self.emit_before(warnings, release.extent.end(), &owner)
            }
        }
    }

    fn history_id(
        &mut self,
        table: &str,
        key: &str,
        set: CatalogSetId,
        order: usize,
        extent: XmlSourceExtent,
    ) -> crate::Result<i64> {
        if !self.pending() {
            return self.seek_row(table, key, ("set_id", set.as_i64()), order, extent);
        }
        Ok(sql_query(format!("INSERT INTO {table}(set_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,?,?,?,?) RETURNING {key} AS value"))
            .bind::<BigInt,_>(set.as_i64()).bind::<BigInt,_>(ordinal(order)?)
            .bind::<BigInt,_>(extent.start().line).bind::<BigInt,_>(extent.start().column)
            .bind::<BigInt,_>(extent.end().line).bind::<BigInt,_>(extent.end().column).get_result::<Id>(self.conn)?.value)
    }

    fn history_children<'a>(
        &mut self,
        set: CatalogSetId,
        history_id: i64,
        parent: &NoIntroDiagnosticOwner,
        children: impl Iterator<Item = HistoryChild<'a>>,
        file_order: &mut usize,
        warnings: &mut RecoveryCursor<'_>,
    ) -> crate::Result<()> {
        for child in children {
            let (order, extent) = child.position();
            self.emit_before(warnings, extent.start(), parent)?;
            let owner = self.child(set, history_id, child, file_order)?;
            debug_assert_eq!(owner.link().extent, extent);
            debug_assert_eq!(order, child.position().0);
            self.emit_before(warnings, extent.end(), &owner)?;
        }
        Ok(())
    }

    fn child(
        &mut self,
        set: CatalogSetId,
        id: i64,
        child: HistoryChild<'_>,
        file_order: &mut usize,
    ) -> crate::Result<NoIntroDiagnosticOwner> {
        let (order, extent) = child.position();
        let common = [
            id,
            ordinal(order)?,
            extent.start().line,
            extent.start().column,
            extent.end().line,
            extent.end().column,
        ];
        match child {
            HistoryChild::DumpDetails(details) => self.dump_details(id, details, common),
            HistoryChild::DumpSerials(serials) => {
                self.row(
                    NativeTable::DumpSerials,
                    ("dump_source_id", id),
                    order,
                    extent,
                    &common,
                    &dump_serials_fields(serials),
                )?;
                Ok(NoIntroDiagnosticOwner::DumpSerials {
                    id: NoIntroDumpSourceId::try_from(id)?,
                    extent,
                })
            }
            HistoryChild::ReleaseDetails(details) => self.release_details(id, details, common),
            HistoryChild::ReleaseSerials(serials) => {
                self.row(
                    NativeTable::ReleaseSerials,
                    ("release_id", id),
                    order,
                    extent,
                    &common,
                    &release_serials_fields(serials),
                )?;
                Ok(NoIntroDiagnosticOwner::ReleaseSerials {
                    id: NoIntroReleaseId::try_from(id)?,
                    extent,
                })
            }
            HistoryChild::DumpFile(file) => {
                let occurrence = self.dump_file(
                    FilePlacement {
                        parent: ("dump_source_id", id),
                        set,
                        source_order: order,
                        extent,
                        occurrence_order: *file_order,
                    },
                    file,
                )?;
                *file_order = file_order
                    .checked_add(1)
                    .ok_or_else(|| invalid_native("file order overflow"))?;
                Ok(NoIntroDiagnosticOwner::DumpFile {
                    id: occurrence,
                    extent,
                })
            }
            HistoryChild::ReleaseFile(file) => {
                let occurrence = self.release_file(
                    FilePlacement {
                        parent: ("release_id", id),
                        set,
                        source_order: order,
                        extent,
                        occurrence_order: *file_order,
                    },
                    file,
                )?;
                *file_order = file_order
                    .checked_add(1)
                    .ok_or_else(|| invalid_native("file order overflow"))?;
                Ok(NoIntroDiagnosticOwner::ReleaseFile {
                    id: occurrence,
                    extent,
                })
            }
        }
    }

    fn dump_details(
        &mut self,
        id: i64,
        details: &SourceDetails,
        common: [i64; 6],
    ) -> crate::Result<NoIntroDiagnosticOwner> {
        self.row(
            NativeTable::DumpDetails,
            ("dump_source_id", id),
            details.source_order,
            details.extent,
            &details_keys(common, details.opening_end),
            &dump_details_fields(details),
        )?;
        self.validate_opening_end(NativeTable::DumpDetails, id, details.opening_end)?;
        Ok(NoIntroDiagnosticOwner::DumpDetails {
            id: NoIntroDumpSourceId::try_from(id)?,
            extent: details.extent,
        })
    }

    fn release_details(
        &mut self,
        id: i64,
        details: &ReleaseDetails,
        common: [i64; 6],
    ) -> crate::Result<NoIntroDiagnosticOwner> {
        self.row(
            NativeTable::ReleaseDetails,
            ("release_id", id),
            details.source_order,
            details.extent,
            &details_keys(common, details.opening_end),
            &release_details_fields(details),
        )?;
        self.validate_opening_end(NativeTable::ReleaseDetails, id, details.opening_end)?;
        if self.pending() {
            insert_nfo_hashes(self.conn, id, details)?;
        }
        Ok(NoIntroDiagnosticOwner::ReleaseDetails {
            id: NoIntroReleaseId::try_from(id)?,
            extent: details.extent,
        })
    }

    fn dump_file(
        &mut self,
        placement: FilePlacement,
        file: &SourceFile,
    ) -> crate::Result<OccurrenceId> {
        let occurrence = self.file(NativeTable::DumpFile, placement, &dump_file_fields(file))?;
        if self.pending() {
            insert_file_hashes(
                self.conn,
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
                let hash = insert_digest(self.conn, "sha256", origin)?;
                insert_file_digest_row(self.conn, FileTable::Dump, occurrence, 4, hash, origin)?;
                record_occurrence_digest_assertions(
                    self.conn,
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
        }
        Ok(occurrence)
    }

    fn release_file(
        &mut self,
        placement: FilePlacement,
        file: &ReleaseFile,
    ) -> crate::Result<OccurrenceId> {
        let occurrence = self.file(
            NativeTable::ReleaseFile,
            placement,
            &release_file_fields(file),
        )?;
        if self.pending() {
            insert_file_hashes(
                self.conn,
                FileTable::Release,
                occurrence,
                [
                    file.crc32.as_ref(),
                    file.md5.as_ref(),
                    file.sha1.as_ref(),
                    file.sha256.as_ref(),
                ],
            )?;
        }
        Ok(occurrence)
    }

    fn file(
        &mut self,
        table: NativeTable,
        placement: FilePlacement,
        fields: &[Field<'_>],
    ) -> crate::Result<OccurrenceId> {
        let FilePlacement {
            parent,
            set,
            source_order: order,
            extent,
            occurrence_order: file_order,
        } = placement;
        let kind = match table {
            NativeTable::DumpFile => "no_intro_database_source_file",
            NativeTable::ReleaseFile => "no_intro_database_release_file",
            _ => return Err(invalid_native("not a native file table")),
        };
        let occurrence = if self.pending() {
            let occurrence = insert_occurrence(self.conn, set, file_order, kind)?;
            self.row(
                table,
                parent,
                order,
                extent,
                &[
                    occurrence.database_value(),
                    parent.1,
                    set.as_i64(),
                    ordinal(order)?,
                    extent.start().line,
                    extent.start().column,
                    extent.end().line,
                    extent.end().column,
                ],
                fields,
            )?;
            occurrence
        } else {
            OccurrenceId::try_from(self.seek_row(
                table.table(),
                table.owner_column(),
                parent,
                order,
                extent,
            )?)?
        };
        sql_query("SELECT 1 AS value FROM asset_occurrences WHERE typeof(occurrence_id)='integer' AND occurrence_id=? AND typeof(record_id)='integer' AND record_id=? AND typeof(occurrence_order)='integer' AND occurrence_order=? AND typeof(claim_kind)='text' AND claim_kind=?")
            .bind::<BigInt,_>(occurrence.database_value()).bind::<BigInt,_>(set.as_i64()).bind::<BigInt,_>(ordinal(file_order)?).bind::<Text,_>(kind).get_result::<Id>(self.conn)?;
        sql_query(format!("SELECT 1 AS value FROM {} WHERE occurrence_id=? AND typeof(set_id)='integer' AND set_id=?", table.table()))
            .bind::<BigInt,_>(occurrence.database_value()).bind::<BigInt,_>(set.as_i64()).get_result::<Id>(self.conn)?;
        Ok(occurrence)
    }

    fn validate_opening_end(
        &mut self,
        table: NativeTable,
        id: i64,
        end: crate::logiqx::RecordLocation,
    ) -> crate::Result<()> {
        sql_query(format!("SELECT 1 AS value FROM {} WHERE {}=? AND typeof(opening_end_line)='integer' AND typeof(opening_end_column)='integer' AND opening_end_line=? AND opening_end_column=?", table.table(), table.owner_column()))
            .bind::<BigInt,_>(id).bind::<BigInt,_>(end.line).bind::<BigInt,_>(end.column).get_result::<Id>(self.conn)?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct FilePlacement {
    parent: (&'static str, i64),
    set: CatalogSetId,
    source_order: usize,
    extent: XmlSourceExtent,
    occurrence_order: usize,
}

const fn details_keys(common: [i64; 6], opening_end: RecordLocation) -> [i64; 8] {
    [
        common[0],
        common[1],
        common[2],
        common[3],
        common[4],
        common[5],
        opening_end.line,
        opening_end.column,
    ]
}

fn invalid_native(message: &str) -> crate::Error {
    crate::Error::DatabaseSchema(message.into())
}

#[derive(Clone, Copy)]
enum HistoryChild<'a> {
    DumpDetails(&'a SourceDetails),
    DumpSerials(&'a SourceSerials),
    DumpFile(&'a SourceFile),
    ReleaseDetails(&'a ReleaseDetails),
    ReleaseSerials(&'a ReleaseSerials),
    ReleaseFile(&'a ReleaseFile),
}

impl HistoryChild<'_> {
    const fn position(self) -> (usize, XmlSourceExtent) {
        match self {
            Self::DumpDetails(value) => (value.source_order, value.extent),
            Self::DumpSerials(value) => (value.source_order, value.extent),
            Self::DumpFile(value) => (value.source_order, value.extent),
            Self::ReleaseDetails(value) => (value.source_order, value.extent),
            Self::ReleaseSerials(value) => (value.source_order, value.extent),
            Self::ReleaseFile(value) => (value.source_order, value.extent),
        }
    }
}

fn ordered_history_children<'a>(
    mut details: Option<HistoryChild<'a>>,
    mut serials: Option<HistoryChild<'a>>,
    files: impl Iterator<Item = HistoryChild<'a>>,
) -> impl Iterator<Item = HistoryChild<'a>> {
    let mut files = files.peekable();
    std::iter::from_fn(move || {
        let orders = [
            details.map(|child| child.position().0),
            serials.map(|child| child.position().0),
            files.peek().map(|child| child.position().0),
        ];
        let next = orders
            .into_iter()
            .enumerate()
            .filter_map(|(index, order)| order.map(|order| (index, order)))
            .min_by_key(|(_, order)| *order)?;
        match next.0 {
            0 => details.take(),
            1 => serials.take(),
            _ => files.next(),
        }
    })
}

fn insert_nfo_hashes(
    conn: &mut SqliteConnection,
    id: i64,
    details: &ReleaseDetails,
) -> crate::Result<()> {
    for (name, digest) in [
        ("nfo_crc32", details.nfo_crc32.as_ref()),
        ("nfocrc", details.nfocrc.as_ref()),
    ] {
        if let Some(digest) = digest {
            let hash = insert_digest(conn, "crc32", digest)?;
            sql_query("INSERT INTO no_intro_release_nfo_hashes(release_id,source_hash_field,hash_id,presence,scope,invalid_literal) VALUES(?,?,?,'present','nfo_companion',?)")
                .bind::<BigInt,_>(id).bind::<Text,_>(name).bind::<Nullable<BigInt>,_>(hash)
                .bind::<Nullable<Text>,_>(hash.is_none().then_some(digest.source.as_str())).execute(conn)?;
        }
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
