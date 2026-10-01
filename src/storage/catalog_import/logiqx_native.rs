use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

use crate::{
    domain::{CatalogSetId, SnapshotKey},
    logiqx::{DataFile, Game},
};

#[derive(Clone)]
enum DeclaredValue<T> {
    Default(T),
    Explicit(T),
}

impl<T> DeclaredValue<T> {
    const fn value(&self) -> &T {
        match self {
            Self::Default(value) | Self::Explicit(value) => value,
        }
    }

    const fn was_present(&self) -> bool {
        matches!(self, Self::Explicit(_))
    }
}

const fn declared_value(value: String, was_present: bool) -> DeclaredValue<String> {
    if was_present {
        DeclaredValue::Explicit(value)
    } else {
        DeclaredValue::Default(value)
    }
}

#[derive(Clone)]
struct ClrMameProOptions {
    source_order: Option<usize>,
    line: i64,
    column: i64,
    header: Option<String>,
    forcemerging: DeclaredValue<String>,
    forcenodump: DeclaredValue<String>,
    forcepacking: DeclaredValue<String>,
}

#[derive(Clone)]
struct RomCenterOptions {
    source_order: Option<usize>,
    line: i64,
    column: i64,
    plugin: Option<String>,
    rommode: DeclaredValue<String>,
    biosmode: DeclaredValue<String>,
    samplemode: DeclaredValue<String>,
    lockrommode: DeclaredValue<String>,
    lockbiosmode: DeclaredValue<String>,
    locksamplemode: DeclaredValue<String>,
}

pub(super) struct DocumentDetails {
    clrmamepro: Option<ClrMameProOptions>,
    romcenter: Option<RomCenterOptions>,
}

impl DocumentDetails {
    pub(super) fn from_data_file(data_file: &DataFile) -> crate::Result<Self> {
        let header = data_file.header_opt();
        let clrmamepro = data_file.clrmamepro_options_opt().map(|options| {
            let location = options.location();
            ClrMameProOptions {
                source_order: header.and_then(|header| header.child_source_order(location)),
                line: location.line,
                column: location.column,
                header: options.header().map(str::to_owned),
                forcemerging: declared_value(
                    options.forcemerging().to_owned(),
                    options.forcemerging_was_explicit(),
                ),
                forcenodump: declared_value(
                    options.forcenodump().to_owned(),
                    options.forcenodump_was_explicit(),
                ),
                forcepacking: declared_value(
                    options.forcepacking().to_owned(),
                    options.forcepacking_was_explicit(),
                ),
            }
        });
        let romcenter = data_file.romcenter_options_opt().map(|options| {
            let location = options.location();
            RomCenterOptions {
                source_order: header.and_then(|header| header.child_source_order(location)),
                line: location.line,
                column: location.column,
                plugin: options.plugin().map(str::to_owned),
                rommode: declared_value(
                    options.rommode().to_owned(),
                    options.rommode_was_explicit(),
                ),
                biosmode: declared_value(
                    options.biosmode().to_owned(),
                    options.biosmode_was_explicit(),
                ),
                samplemode: declared_value(
                    options.samplemode().to_owned(),
                    options.samplemode_was_explicit(),
                ),
                lockrommode: declared_value(
                    options.lockrommode().to_owned(),
                    options.lockrommode_was_explicit(),
                ),
                lockbiosmode: declared_value(
                    options.lockbiosmode().to_owned(),
                    options.lockbiosmode_was_explicit(),
                ),
                locksamplemode: declared_value(
                    options.locksamplemode().to_owned(),
                    options.locksamplemode_was_explicit(),
                ),
            }
        });
        let details = Self {
            clrmamepro,
            romcenter,
        };
        if let Some(options) = &details.clrmamepro {
            checked_source_order(options.source_order, "ClrMamePro options")?;
        }
        if let Some(options) = &details.romcenter {
            checked_source_order(options.source_order, "RomCenter options")?;
        }
        Ok(details)
    }

    pub(super) fn insert(
        &self,
        conn: &mut SqliteConnection,
        snapshot_key: &SnapshotKey,
    ) -> crate::Result<()> {
        if let Some(options) = &self.clrmamepro {
            sql_query(
                "INSERT INTO logiqx_clrmamepro_options (
                    snapshot_key, source_order, source_line, source_column,
                    header, header_was_present,
                    forcemerging, forcemerging_was_present,
                    forcenodump, forcenodump_was_present,
                    forcepacking, forcepacking_was_present
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(snapshot_key.as_str())
            .bind::<BigInt, _>(checked_source_order(
                options.source_order,
                "ClrMamePro options",
            )?)
            .bind::<BigInt, _>(options.line)
            .bind::<BigInt, _>(options.column)
            .bind::<Nullable<Text>, _>(options.header.as_deref())
            .bind::<Bool, _>(options.header.is_some())
            .bind::<Text, _>(options.forcemerging.value())
            .bind::<Bool, _>(options.forcemerging.was_present())
            .bind::<Text, _>(options.forcenodump.value())
            .bind::<Bool, _>(options.forcenodump.was_present())
            .bind::<Text, _>(options.forcepacking.value())
            .bind::<Bool, _>(options.forcepacking.was_present())
            .execute(conn)?;
        }

        if let Some(options) = &self.romcenter {
            sql_query(
                "INSERT INTO logiqx_romcenter_options (
                    snapshot_key, source_order, source_line, source_column,
                    plugin, plugin_was_present,
                    rommode, rommode_was_present,
                    biosmode, biosmode_was_present,
                    samplemode, samplemode_was_present,
                    lockrommode, lockrommode_was_present,
                    lockbiosmode, lockbiosmode_was_present,
                    locksamplemode, locksamplemode_was_present
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(snapshot_key.as_str())
            .bind::<BigInt, _>(checked_source_order(
                options.source_order,
                "RomCenter options",
            )?)
            .bind::<BigInt, _>(options.line)
            .bind::<BigInt, _>(options.column)
            .bind::<Nullable<Text>, _>(options.plugin.as_deref())
            .bind::<Bool, _>(options.plugin.is_some())
            .bind::<Text, _>(options.rommode.value())
            .bind::<Bool, _>(options.rommode.was_present())
            .bind::<Text, _>(options.biosmode.value())
            .bind::<Bool, _>(options.biosmode.was_present())
            .bind::<Text, _>(options.samplemode.value())
            .bind::<Bool, _>(options.samplemode.was_present())
            .bind::<Text, _>(options.lockrommode.value())
            .bind::<Bool, _>(options.lockrommode.was_present())
            .bind::<Text, _>(options.lockbiosmode.value())
            .bind::<Bool, _>(options.lockbiosmode.was_present())
            .bind::<Text, _>(options.locksamplemode.value())
            .bind::<Bool, _>(options.locksamplemode.was_present())
            .execute(conn)?;
        }
        Ok(())
    }
}

struct CommentDetails {
    comment_order: i64,
    source_order: i64,
    text: String,
    line: i64,
    column: i64,
}

struct ReleaseDetails {
    release_order: i64,
    source_order: i64,
    name: String,
    region: String,
    language: Option<String>,
    date: Option<String>,
    default: DeclaredValue<String>,
    line: i64,
    column: i64,
}

struct BiosSetDetails {
    bios_order: i64,
    source_order: i64,
    name: String,
    description: String,
    is_default: DeclaredValue<String>,
    line: i64,
    column: i64,
}

struct ArchiveReferenceDetails {
    archive_order: i64,
    source_order: i64,
    archive_name: String,
    line: i64,
    column: i64,
}

pub(super) struct GameDetails {
    comments: Vec<CommentDetails>,
    releases: Vec<ReleaseDetails>,
    bios_sets: Vec<BiosSetDetails>,
    archive_references: Vec<ArchiveReferenceDetails>,
}

impl GameDetails {
    pub(super) fn from_game(game: &Game) -> crate::Result<Self> {
        let comments = game
            .comments()
            .iter()
            .enumerate()
            .map(|(order, comment)| -> crate::Result<_> {
                let location = comment.location();
                Ok(CommentDetails {
                    comment_order: checked_family_order(order, "comments")?,
                    source_order: checked_source_order(
                        game.child_source_order(location),
                        "comment",
                    )?,
                    text: comment.text().to_owned(),
                    line: location.line,
                    column: location.column,
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let releases = game
            .releases()
            .iter()
            .enumerate()
            .map(|(order, release)| -> crate::Result<_> {
                let location = release.location();
                Ok(ReleaseDetails {
                    release_order: checked_family_order(order, "releases")?,
                    source_order: checked_source_order(
                        game.child_source_order(location),
                        "release",
                    )?,
                    name: release.name().to_owned(),
                    region: release.region().to_owned(),
                    language: release.language().map(str::to_owned),
                    date: release.date().map(str::to_owned),
                    default: declared_value(
                        release.default().to_owned(),
                        release.default_was_explicit(),
                    ),
                    line: location.line,
                    column: location.column,
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let bios_sets = game
            .bios_sets()
            .iter()
            .enumerate()
            .map(|(order, bios_set)| -> crate::Result<_> {
                let location = bios_set.location();
                Ok(BiosSetDetails {
                    bios_order: checked_family_order(order, "BIOS sets")?,
                    source_order: checked_source_order(
                        game.child_source_order(location),
                        "BIOS set",
                    )?,
                    name: bios_set.name().to_owned(),
                    description: bios_set.description().to_owned(),
                    is_default: declared_value(
                        bios_set.default().to_owned(),
                        bios_set.default_was_explicit(),
                    ),
                    line: location.line,
                    column: location.column,
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let archive_references = game
            .archives()
            .iter()
            .enumerate()
            .map(|(order, archive)| -> crate::Result<_> {
                let location = archive.location();
                Ok(ArchiveReferenceDetails {
                    archive_order: checked_family_order(order, "archive references")?,
                    source_order: checked_source_order(
                        game.child_source_order(location),
                        "archive reference",
                    )?,
                    archive_name: archive.name().to_owned(),
                    line: location.line,
                    column: location.column,
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;
        Ok(Self {
            comments,
            releases,
            bios_sets,
            archive_references,
        })
    }

    pub(super) fn insert(
        &self,
        conn: &mut SqliteConnection,
        set_id: CatalogSetId,
    ) -> crate::Result<()> {
        for comment in &self.comments {
            sql_query(
                "INSERT INTO logiqx_game_comments \
                 (set_id, comment_order, source_order, comment_text, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id.as_i64())
            .bind::<BigInt, _>(comment.comment_order)
            .bind::<BigInt, _>(comment.source_order)
            .bind::<Text, _>(&comment.text)
            .bind::<BigInt, _>(comment.line)
            .bind::<BigInt, _>(comment.column)
            .execute(conn)?;
        }

        for release in &self.releases {
            sql_query(
                "INSERT INTO logiqx_releases (
                    set_id, release_order, source_order, name, region, language, date,
                    \"default\", default_was_present, source_line, source_column
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id.as_i64())
            .bind::<BigInt, _>(release.release_order)
            .bind::<BigInt, _>(release.source_order)
            .bind::<Text, _>(&release.name)
            .bind::<Text, _>(&release.region)
            .bind::<Nullable<Text>, _>(release.language.as_deref())
            .bind::<Nullable<Text>, _>(release.date.as_deref())
            .bind::<Text, _>(release.default.value())
            .bind::<Bool, _>(release.default.was_present())
            .bind::<BigInt, _>(release.line)
            .bind::<BigInt, _>(release.column)
            .execute(conn)?;
        }

        for bios_set in &self.bios_sets {
            sql_query(
                "INSERT INTO logiqx_bios_sets (
                    set_id, bios_order, source_order, name, description, is_default,
                    default_was_present, source_line, source_column
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id.as_i64())
            .bind::<BigInt, _>(bios_set.bios_order)
            .bind::<BigInt, _>(bios_set.source_order)
            .bind::<Text, _>(&bios_set.name)
            .bind::<Text, _>(&bios_set.description)
            .bind::<Text, _>(bios_set.is_default.value())
            .bind::<Bool, _>(bios_set.is_default.was_present())
            .bind::<BigInt, _>(bios_set.line)
            .bind::<BigInt, _>(bios_set.column)
            .execute(conn)?;
        }

        for archive in &self.archive_references {
            sql_query(
                "INSERT INTO logiqx_archive_references \
                 (set_id, archive_order, source_order, archive_name, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id.as_i64())
            .bind::<BigInt, _>(archive.archive_order)
            .bind::<BigInt, _>(archive.source_order)
            .bind::<Text, _>(&archive.archive_name)
            .bind::<BigInt, _>(archive.line)
            .bind::<BigInt, _>(archive.column)
            .execute(conn)?;
        }
        Ok(())
    }
}

fn checked_family_order(order: usize, family: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many Logiqx {family}")))
}

fn checked_source_order(order: Option<usize>, family: &str) -> crate::Result<i64> {
    let order = order.ok_or_else(|| {
        crate::Error::InvalidPath(format!("missing Logiqx {family} source order"))
    })?;
    i64::try_from(order)
        .map_err(|_| crate::Error::InvalidPath("too many Logiqx game children".into()))
}
