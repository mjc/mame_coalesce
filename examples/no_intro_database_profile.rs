//! Exercise the native database-export reader without retaining a catalog tree.

use std::env;

use mame_coalesce::{
    Error,
    no_intro_db_xml::{self, DatabaseGame, EnvelopeKind, NoIntroDatabaseMode, SourceOrRelease},
};

struct Counts {
    envelope: EnvelopeKind,
    games: usize,
    archives: usize,
    sources: usize,
    releases: usize,
    source_files: usize,
    release_files: usize,
}

impl Counts {
    const fn new(envelope: EnvelopeKind) -> Self {
        Self {
            envelope,
            games: 0,
            archives: 0,
            sources: 0,
            releases: 0,
            source_files: 0,
            release_files: 0,
        }
    }

    fn include(&mut self, game: DatabaseGame) {
        self.games += 1;
        self.archives += game.archives.len();
        for owner in game.source_or_release {
            match owner {
                SourceOrRelease::Source(source) => {
                    self.sources += 1;
                    self.source_files += source.files.len();
                }
                SourceOrRelease::Release(release) => {
                    self.releases += 1;
                    self.release_files += release.files.len();
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let mode = match arguments.next().as_deref() {
        Some("no-intro-database-xml-compatible") => NoIntroDatabaseMode::ObservedCompatible,
        Some("no-intro-database-xml-nul-compatible") => NoIntroDatabaseMode::NullRecoveryCompatible,
        _ => return Err("provide an explicit database-export interpretation and XML paths".into()),
    };
    let mut succeeded = 0;
    let mut failed = 0;
    for path in arguments {
        let bytes = std::fs::read(&path)?;
        let result = no_intro_db_xml::read_with::<_, Error>(
            &bytes,
            mode,
            |document| Ok(Counts::new(document.envelope)),
            |counts, game| {
                counts.include(game);
                Ok(())
            },
        );
        match result {
            Ok(validated) => {
                let warnings = validated.recovery_warnings().count();
                for warning in validated.recovery_warnings() {
                    println!(
                        "RECOVERY document={path:?} line={} column={} replaced=U+{:04X}",
                        warning.location.line,
                        warning.location.column,
                        u32::from(warning.replaced),
                    );
                }
                let counts = validated.into_inner();
                println!(
                    "PARSED document={path:?} envelope={:?} games={} archives={} sources={} releases={} source_files={} release_files={} warnings={warnings}",
                    counts.envelope,
                    counts.games,
                    counts.archives,
                    counts.sources,
                    counts.releases,
                    counts.source_files,
                    counts.release_files,
                );
                succeeded += 1;
            }
            Err(error) => {
                eprintln!("PARSE_ERROR document={path:?} error={error}");
                failed += 1;
            }
        }
    }
    println!(
        "format={} succeeded={succeeded} failed={failed}",
        mode.format()
    );
    if succeeded + failed == 0 {
        return Err("provide at least one XML document".into());
    }
    if failed > 0 {
        return Err(format!("{failed} database-export documents failed validation").into());
    }
    Ok(())
}
