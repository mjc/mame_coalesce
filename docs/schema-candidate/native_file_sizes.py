"""Query-only numeric projections of native whole-file size declarations."""


I64_MAX = "9223372036854775807"


def _decimal_i64_sql(expression: str, *, allow_plus: bool = False) -> str:
    """Project ASCII decimal within SQLite INTEGER, optionally allowing ``+``."""
    value = f"({expression})"
    digits = (
        f"CASE WHEN substr({value}, 1, 1) = '+' THEN substr({value}, 2) ELSE {value} END"
        if allow_plus else value
    )
    significant = f"ltrim({digits}, '0')"
    return f"""CASE
    WHEN typeof({value}) = 'text'
     AND instr(CAST({value} AS BLOB), X'00') = 0
     AND length({digits}) > 0
     AND {digits} NOT GLOB '*[^0-9]*'
     AND (length({significant}) < 19 OR
          (length({significant}) = 19 AND {significant} COLLATE BINARY <= '{I64_MAX}'))
    THEN CAST(CASE WHEN {significant} = '' THEN '0' ELSE {significant} END AS INTEGER)
END"""


def _xs_unsigned_int_sql(expression: str) -> str:
    """Mirror strict flat-DAT xs:unsignedInt parsing, narrowed to u32."""
    value = f"({expression})"
    lexical = f"trim({value}, char(9) || char(10) || char(13) || ' ')"
    digits = (
        f"CASE WHEN substr({lexical}, 1, 1) IN ('+', '-') "
        f"THEN substr({lexical}, 2) ELSE {lexical} END"
    )
    significant = f"ltrim({digits}, '0')"
    return f"""CASE
    WHEN typeof({value}) = 'text'
     AND instr(CAST({value} AS BLOB), X'00') = 0
     AND length({digits}) > 0
     AND {digits} NOT GLOB '*[^0-9]*'
     AND (substr({lexical}, 1, 1) <> '-' OR {significant} = '')
     AND (length({significant}) < 10 OR
          (length({significant}) = 10 AND {significant} COLLATE BINARY <= '4294967295'))
    THEN CAST(CASE WHEN {significant} = '' THEN '0' ELSE {significant} END AS INTEGER)
END"""


def _native_size_scalar(table: str, raw_column: str, value_expression: str) -> str:
    raw = f"{table}.{raw_column}"
    return f"""(SELECT CASE WHEN {raw} IS NULL THEN 'omitted'
                 WHEN ({value_expression}) IS NULL THEN 'unusable'
                 ELSE 'value' END
          FROM {table}
         WHERE {table}.media_entry_id = media.media_entry_id)"""


def sql() -> str:
    """Return a flattened, primary-key-addressable file-size view.

    Every branch is keyed by the native media entry. The software-list branch
    reuses the existing full-file-length derivation; it never treats a missing
    derivation as an optional size. These values describe declarations only
    and do not qualify file identity. ``unusable`` means unusable for this
    signed-integer comparison, not necessarily invalid source syntax: native
    P/C accepts the full u64 range and compatible DAT/export retain arbitrary
    text. Their decimal comparison is intentionally conservative.
    """
    strict_dat = "rules.dialect IN ('no-intro-dat-v3-strict', 'no-intro-dat-v4-strict')"
    compatible_dat = "rules.dialect IN ('no-intro-dat-v3-compatible', 'no-intro-dat-v4-compatible')"
    strict_dat_size = _xs_unsigned_int_sql("rom.size_text")
    compatible_dat_size = _decimal_i64_sql("rom.size_text")
    dat_size = f"CASE WHEN {strict_dat} THEN ({strict_dat_size}) WHEN {compatible_dat} THEN ({compatible_dat_size}) END"

    mame_value = _decimal_i64_sql("mame_roms.size_text")
    cmp_value = "clrmamepro_roms.size_value"
    logiqx_value = "logiqx_roms.size_value"
    pc_value = "no_intro_pc_file_claims.size_i64"
    dump_value = _decimal_i64_sql("no_intro_dump_files.size_text")
    release_value = _decimal_i64_sql("no_intro_release_files.size_text")
    software_value = (
        "(SELECT length.byte_length FROM candidate_software_file_lengths AS length "
        "WHERE length.media_entry_id = media.media_entry_id)"
    )

    def value_case(kind: str, table: str, expression: str) -> str:
        return (
            f"WHEN source.element_kind = '{kind}' THEN "
            f"(SELECT {expression} FROM {table} "
            "WHERE media_entry_id = media.media_entry_id)"
        )

    def state_case(kind: str, state_expression: str) -> str:
        return f"WHEN source.element_kind = '{kind}' THEN coalesce(({state_expression}), 'unusable')"

    size_states = [
        state_case("mame_rom", _native_size_scalar("mame_roms", "size_text", mame_value)),
        "WHEN source.element_kind = 'software_rom_entry' THEN "
        f"CASE WHEN ({software_value}) IS NULL THEN 'unusable' ELSE 'value' END",
        state_case("clrmamepro_rom", _native_size_scalar("clrmamepro_roms", "size_text", cmp_value)),
        state_case("logiqx_rom", _native_size_scalar("logiqx_roms", "size_text", logiqx_value)),
        "WHEN source.element_kind = 'no_intro_dat_rom' THEN "
        f"coalesce((SELECT CASE WHEN rom.size_text IS NULL THEN 'omitted' "
        f"WHEN ({dat_size}) IS NULL THEN 'unusable' ELSE 'value' END "
        "FROM no_intro_dat_rom_claims AS rom "
        "WHERE rom.media_entry_id = media.media_entry_id), 'unusable')",
        state_case("no_intro_pc_rom", _native_size_scalar(
            "no_intro_pc_file_claims", "size_text", pc_value)),
        state_case("no_intro_export_source_file", _native_size_scalar(
            "no_intro_dump_files", "size_text", dump_value)),
        state_case("no_intro_export_release_file", _native_size_scalar(
            "no_intro_release_files", "size_text", release_value)),
    ]

    return f"""CREATE VIEW candidate_native_file_sizes AS
SELECT media.media_entry_id,
       CASE source.element_kind
           WHEN 'software_rom_entry' THEN 'file_length'
           ELSE 'size'
       END AS source_size_field,
       CASE
           {value_case('mame_rom', 'mame_roms', mame_value)}
           WHEN source.element_kind = 'software_rom_entry' THEN {software_value}
           {value_case('clrmamepro_rom', 'clrmamepro_roms', cmp_value)}
           {value_case('logiqx_rom', 'logiqx_roms', logiqx_value)}
           WHEN source.element_kind = 'no_intro_dat_rom' THEN
               (SELECT {dat_size} FROM no_intro_dat_rom_claims AS rom
                WHERE rom.media_entry_id = media.media_entry_id)
           {value_case('no_intro_pc_rom', 'no_intro_pc_file_claims', pc_value)}
           {value_case('no_intro_export_source_file', 'no_intro_dump_files', dump_value)}
           {value_case('no_intro_export_release_file', 'no_intro_release_files', release_value)}
       END AS byte_length,
       CASE
           {' '.join(size_states)}
       END AS size_state
FROM catalog_media_entries AS media
JOIN catalog_source_elements AS source
  ON source.source_element_id = media.media_entry_id
JOIN catalog_editions AS edition
  ON edition.edition_id = source.edition_id
JOIN catalog_reading_rules AS rules
  ON rules.reading_rules_id = edition.reading_rules_id
WHERE source.element_kind IN (
    'mame_rom', 'software_rom_entry', 'clrmamepro_rom', 'logiqx_rom',
    'no_intro_dat_rom', 'no_intro_pc_rom',
    'no_intro_export_source_file', 'no_intro_export_release_file'
)
  AND (source.element_kind <> 'software_rom_entry' OR EXISTS (
       SELECT 1 FROM candidate_software_file_lengths AS software
       WHERE software.media_entry_id = media.media_entry_id
  ))
  AND (source.element_kind <> 'no_intro_dat_rom' OR
       (rules.format_family = 'no_intro_dat' AND rules.dialect IN (
           'no-intro-dat-v3-strict', 'no-intro-dat-v4-strict',
           'no-intro-dat-v3-compatible', 'no-intro-dat-v4-compatible'
       )));"""
