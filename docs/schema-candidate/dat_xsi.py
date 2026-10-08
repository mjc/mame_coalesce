"""Query-only No-Intro DAT XSI semantics for the executable design candidate.

The closed routes below refer to native typed owners. The views are audit
projections, not stored metadata or a second copy of source values. Namespace
binding is a checked-reader obligation: spelling cannot prove that a prefix was
bound to XML Schema (or XML Schema Instance) at the source element.
"""

from xml_numeric_sql import xml_integer_sql
from xml_text_sql import (
    xml_collapse_sql, xml_language_valid_sql, xml_name_valid_sql,
    xml_qname_valid_sql,
)


# Native table, native key, XSI relation suffix, simple text column (if any).
ROUTES = (
    ("no_intro_dat_documents", "edition_id", "document", None),
    ("no_intro_dat_headers", "header_id", "header", None),
    ("no_intro_dat_header_text_children", "source_element_id", "header_text_child", "value_text"),
    ("no_intro_dat_clrmamepro_options", "source_element_id", "clrmamepro", None),
    ("no_intro_dat_romcenter_options", "source_element_id", "romcenter", None),
    ("no_intro_dat_games", "set_id", "game", None),
    ("no_intro_dat_game_descriptions", "source_element_id", "game_description", "description_text"),
    ("no_intro_dat_categories", "source_element_id", "category", "category"),
    ("no_intro_dat_identifiers", "source_element_id", "identifier", "identifier"),
    ("no_intro_dat_releases", "source_element_id", "release", None),
    ("no_intro_dat_rom_claims", "media_entry_id", "rom", None),
)
STRING_TYPES = "'string','normalizedString','token','language','Name','NCName','NMTOKEN','ID','IDREF','ENTITY'"
STRICT = "('no-intro-dat-v3-strict','no-intro-dat-v4-strict')"
MODES = "('no-intro-dat-v3-strict','no-intro-dat-v3-compatible','no-intro-dat-v4-strict','no-intro-dat-v4-compatible')"


def _edition_join(table, key):
    if table == "no_intro_dat_documents":
        return "JOIN catalog_editions AS edition ON edition.edition_id=native.edition_id"
    return (f"JOIN catalog_source_elements AS element ON element.source_element_id=native.{key}\n"
            "JOIN catalog_editions AS edition ON edition.edition_id=element.edition_id")


def _edition_owners(table, key, suffix):
    """Anchor an audit at its edition's indexed source-kind range."""
    if suffix == 'document':
        return f"""FROM catalog_editions AS edition
CROSS JOIN {table} AS native ON native.edition_id=edition.edition_id"""
    return f"""FROM catalog_editions AS edition
CROSS JOIN catalog_source_elements AS element
  ON element.edition_id=edition.edition_id AND element.element_kind='no_intro_dat_{suffix}'
CROSS JOIN {table} AS native ON native.{key}=element.source_element_id"""


def guards():
    """Reject bad declarations locally; EOF identity closure stays deferred."""
    result = []
    for native_table, key, suffix, text_column in ROUTES:
        table = f"no_intro_dat_{suffix}_xsi_attributes"
        base = _base_type(suffix, text_column)
        resolved = 'NEW.resolved_type_kind' if text_column else 'NULL'
        row = f"""SELECT NEW.field_kind AS field_kind,NEW.value_text AS value_text,
                   NEW.source_qname AS source_qname,{resolved} AS resolved_type_kind,
                   {base} AS base_type_kind,rules.dialect,rules.format_family
            FROM {native_table} AS native
            {_edition_join(native_table, key)}
            JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
            WHERE native.{key}=NEW.{key}"""
        lexical = f"SELECT *,{xml_collapse_sql('value_text')} AS lexical_value FROM ({row})"
        invalid = ' OR '.join(f'({predicate})' for _, predicate in _declaration_tests())
        invalid += (" OR CASE WHEN field_kind IN ('schemaLocation','noNamespaceSchemaLocation') THEN "
                    f"{_schema_hint_sql('lexical_value', 'field_kind', 'dialect')} IS NOT NULL ELSE 0 END")
        for operation in ("INSERT", "UPDATE"):
            result.append(f"""CREATE TRIGGER candidate_dat_xsi_{suffix}_{operation.lower()}
AFTER {operation} ON {table}
WHEN EXISTS (SELECT 1 FROM ({lexical}) WHERE {invalid})
BEGIN SELECT RAISE(ABORT,'candidate invalid DAT XSI declaration'); END;""")
    return result


def _schema_hint_sql(value, field, dialect):
    """Return a hint problem or NULL, with recursion scoped to one attribute."""
    core = "substr(location,1,min(instr(location||'?', '?'),instr(location||'#', '#'))-1)"
    revisions = []
    for revision in (3, 4):
        filename = f"schema_nointro_datfile_v{revision}.xsd"
        revisions.append(f"WHEN core='{filename}' OR substr(core,-{len(filename)+1}) "
                         f"IN ('/{filename}',char(92)||'{filename}') THEN {revision}")
    return f"""(
WITH RECURSIVE input(value,field,dialect) AS NOT MATERIALIZED (VALUES ({value},{field},{dialect})),
tokens(ordinal,token,rest) AS (
    SELECT 0,'',CASE WHEN field='schemaLocation' THEN value ELSE '' END FROM input
    UNION ALL
    SELECT ordinal+1,substr(rest,1,instr(rest||' ',' ')-1),substr(rest,instr(rest||' ',' ')+1)
    FROM tokens WHERE rest<>''
),
locations(location) AS (
    SELECT token FROM tokens WHERE ordinal>0 AND ordinal%2=0
    UNION ALL
    SELECT value FROM input WHERE field='noNamespaceSchemaLocation'
),
revisions(revision) AS (
    SELECT CASE {' '.join(revisions)} END
    FROM (SELECT {core} AS core FROM locations)
)
SELECT CASE
    WHEN (SELECT count(*) FROM tokens WHERE ordinal>0)%2<>0 THEN 'dat_xsi_schema_pairs'
    WHEN EXISTS (SELECT 1 FROM revisions WHERE revision IS NOT NULL AND revision<>
                 CASE WHEN dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v3-compatible') THEN 3 ELSE 4 END)
    THEN 'dat_xsi_schema_revision'
END FROM input
)"""


def _base_type(suffix, text_column):
    return ("CASE WHEN native.field_kind='id' THEN 'int' ELSE 'string' END"
            if suffix == "header_text_child" else "'string'" if text_column else "NULL")


def _declaration_tests():
    """One source of local-write and global-audit declaration predicates."""
    return (
        ('dat_xsi_reading_rules', f"format_family IS NOT 'no_intro_dat' OR dialect NOT IN {MODES}"),
        ('dat_xsi_source_qname',
         f"NOT ({xml_qname_valid_sql('source_qname')}) OR instr(source_qname,':')=0 "
         "OR substr(source_qname,instr(source_qname,':')+1) IS NOT field_kind"),
        ('dat_xsi_value_nul', "instr(CAST(value_text AS BLOB),X'00')>0"),
        ('dat_xsi_nil', f"field_kind='nil' AND (dialect IN {STRICT} OR lexical_value NOT IN ('false','0'))"),
        ('dat_xsi_type', f"""field_kind='type' AND (
             NOT ({xml_qname_valid_sql('lexical_value')})
             OR substr(lexical_value,instr(lexical_value,':')+1) IS NOT resolved_type_kind
             OR CASE base_type_kind
                  WHEN 'int' THEN resolved_type_kind NOT IN ('int','short','byte')
                  WHEN 'string' THEN resolved_type_kind NOT IN ({STRING_TYPES}) ELSE 1 END)"""),
    )


def sql():
    attributes, simple = [], []
    for table, key, suffix, text_column in ROUTES:
        xsi = f"no_intro_dat_{suffix}_xsi_attributes"
        base = _base_type(suffix, text_column)
        joins = "CROSS JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id"
        attributes.append(f"""SELECT '{suffix}' AS owner_kind, native.{key} AS owner_id,
       edition.edition_id, rules.dialect, rules.format_family, xsi.field_kind, xsi.value_text,
       xsi.source_qname, {('xsi.resolved_type_kind' if text_column else 'NULL')} AS resolved_type_kind,
       {base} AS base_type_kind
{_edition_owners(table, key, suffix)}
CROSS JOIN {xsi} AS xsi ON xsi.{key}=native.{key}
{joins}""")
        if text_column:
            simple.append(f"""SELECT '{suffix}' AS owner_kind, native.{key} AS owner_id,
       edition.edition_id, rules.dialect, native.{text_column} AS value_text,
       coalesce(xsi.resolved_type_kind, {base}) AS effective_type_kind
{_edition_owners(table, key, suffix)}
LEFT JOIN {xsi} AS xsi ON xsi.{key}=native.{key} AND xsi.field_kind='type'
{joins}""")

    declaration_audits = [f"SELECT '{problem}' AS problem,owner_kind,owner_id,edition_id,field_kind "
                          f"FROM candidate_dat_xsi_lexical WHERE {predicate}"
                          for problem, predicate in _declaration_tests()]
    bounds = {"int": (-2147483648, 2147483647), "short": (-32768, 32767), "byte": (-128, 127)}
    value_cases = [f"WHEN '{kind}' THEN {xml_integer_sql('value_text', *limits)} IS NOT NULL"
                   for kind, limits in bounds.items()]
    value_cases += [
        "WHEN 'string' THEN 1 WHEN 'normalizedString' THEN 1 WHEN 'token' THEN 1",
        f"WHEN 'language' THEN {xml_language_valid_sql('collapsed_text')}",
        f"WHEN 'Name' THEN {xml_name_valid_sql('collapsed_text', allow_colon=True)}",
        f"WHEN 'NMTOKEN' THEN {xml_name_valid_sql('collapsed_text', allow_colon=True, require_start=False)}",
        *(f"WHEN '{kind}' THEN {xml_name_valid_sql('collapsed_text')}"
          for kind in ("NCName", "ID", "IDREF")),
        "WHEN 'ENTITY' THEN 0",
    ]
    return f"""CREATE VIEW candidate_dat_xsi_attributes AS
{' UNION ALL '.join(attributes)};

CREATE VIEW candidate_dat_xsi_lexical AS
SELECT *, {xml_collapse_sql('value_text')} AS lexical_value
FROM candidate_dat_xsi_attributes;

CREATE VIEW candidate_dat_simple_values AS
SELECT *, {xml_collapse_sql('value_text')} AS collapsed_text
FROM ({' UNION ALL '.join(simple)});

-- A single partitioned pass avoids replicating a grouped ID materialization
-- for each native UNION branch. Edition is a partition key, so an outer
-- edition filter can move below the window without changing its result.
CREATE VIEW candidate_dat_identity_problems AS
SELECT CASE WHEN effective_type_kind='ID' THEN 'dat_xsi_duplicate_id'
            ELSE 'dat_xsi_unresolved_idref' END AS problem,owner_id,edition_id
FROM (
    SELECT owner_id,edition_id,effective_type_kind,
           sum(effective_type_kind='ID') OVER (
               PARTITION BY edition_id,collapsed_text COLLATE BINARY
           ) AS declarations
    FROM candidate_dat_simple_values
    WHERE dialect IN {STRICT} AND effective_type_kind IN ('ID','IDREF')
)
WHERE (effective_type_kind='ID' AND declarations>1)
   OR (effective_type_kind='IDREF' AND declarations=0);

CREATE VIEW candidate_dat_xsi_declaration_problems AS
{' UNION ALL '.join(declaration_audits)}
UNION ALL
SELECT hint_problem,owner_kind,owner_id,edition_id,field_kind
FROM (SELECT owner_kind,owner_id,edition_id,field_kind,
             {_schema_hint_sql('lexical_value', 'field_kind', 'dialect')} AS hint_problem
      FROM candidate_dat_xsi_lexical WHERE field_kind IN ('schemaLocation','noNamespaceSchemaLocation'))
WHERE hint_problem IS NOT NULL;

CREATE VIEW candidate_dat_xsi_problems AS
SELECT problem,owner_id,edition_id FROM candidate_dat_xsi_declaration_problems
UNION ALL
SELECT 'dat_xsi_simple_value',owner_id,edition_id
FROM candidate_dat_simple_values
WHERE dialect IN {STRICT} AND NOT (CASE effective_type_kind
{' '.join(value_cases)} ELSE 0 END)
UNION ALL
SELECT * FROM candidate_dat_identity_problems;
""" + '\n'.join(guards())
