"""User/derived relationship closure SQL for the isolated schema candidate."""

ORIGINS = (("user", "manual_catalog_relationships"),
           ("derived", "inferred_catalog_relationships"))
ENDPOINTS = (
    ("catalog_set_targets", "catalog_set"),
    ("catalog_media_entry_targets", "catalog_media_entry"),
    ("shared_file_targets", "shared_file"),
    ("declared_hash_targets", "declared_hash"),
    ("observed_content_hash_targets", "observed_content_hash"),
    ("unresolved_catalog_targets", "unresolved_catalog"),
    ("external_catalog_targets", "external_record"),
)
PAYLOAD_ENDPOINTS = (("manual_catalog_relationships", "user"),
                     ("inferred_catalog_relationships", "derived"))


def _lit(value):
    return "'" + value.replace("'", "''") + "'"


def _payload_valid(identity="relationship"):
    choices = []
    for origin, table in ORIGINS:
        other = [f"EXISTS(SELECT 1 FROM {name} AS payload "
                 f"WHERE payload.relationship_id={identity}.relationship_id)"
                 for _, name in ORIGINS if name != table]
        other.append("EXISTS(SELECT 1 FROM reported_catalog_relationships AS payload "
                     f"WHERE payload.relationship_id={identity}.relationship_id)")
        own = (f"EXISTS(SELECT 1 FROM {table} AS payload "
               f"WHERE payload.relationship_id={identity}.relationship_id)")
        choices.append(f"({identity}.origin={_lit(origin)} AND {own} AND "
                       + " AND ".join(f"NOT {condition}" for condition in other) + ")")
    return "(" + " OR ".join(choices) + ")"


def _target_valid(target_id, native_kinds):
    set_kinds, media_kinds = native_kinds
    set_filter = "native.element_kind IN (" + ",".join(_lit(kind) for kind in set_kinds) + ")"
    media_filter = "native.element_kind IN (" + ",".join(_lit(kind) for kind in media_kinds) + ")"
    subtype_union = " UNION ALL ".join(f"SELECT target_id FROM {table}"
                                        for table, _ in ENDPOINTS)
    typed_count = (f"(SELECT count(*) FROM ({subtype_union}) AS subtype "
                   f"WHERE subtype.target_id={target_id})=1")
    actual_set = (
        "target.target_kind='catalog_set' AND EXISTS("
        "SELECT 1 FROM catalog_set_targets AS typed "
        "JOIN catalog_sets AS owner ON owner.set_id=typed.set_id "
        "JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.set_group_id "
        "JOIN catalog_source_elements AS element ON element.source_element_id=owner.set_id "
        "JOIN candidate_native_owners AS native ON native.source_element_id=owner.set_id "
        "AND native.element_kind=element.element_kind "
        "JOIN published_catalog_editions AS publication ON publication.edition_id=groups.edition_id "
        f"WHERE typed.target_id={target_id} AND element.edition_id=groups.edition_id "
        f"AND {set_filter} "
        "AND (SELECT count(*) FROM candidate_native_owners AS actual "
        "WHERE actual.source_element_id=owner.set_id "
        "AND actual.element_kind<>'common_set_placement')=1)")
    actual_media = (
        "target.target_kind='catalog_media_entry' AND EXISTS("
        "SELECT 1 FROM catalog_media_entry_targets AS typed "
        "JOIN catalog_media_entries AS owner ON owner.media_entry_id=typed.media_entry_id "
        "JOIN catalog_source_elements AS element ON element.source_element_id=owner.media_entry_id "
        "JOIN candidate_native_owners AS native ON native.source_element_id=owner.media_entry_id "
        "AND native.element_kind=element.element_kind "
        "JOIN published_catalog_editions AS publication ON publication.edition_id=element.edition_id "
        f"WHERE typed.target_id={target_id} AND {media_filter} "
        "AND (SELECT count(*) FROM candidate_native_owners AS actual "
        "WHERE actual.source_element_id=owner.media_entry_id)=1)")
    other_endpoints = (
        ("shared_file", "shared_file_targets AS typed JOIN shared_catalog_files AS owner "
         "ON owner.file_uuid=typed.file_uuid JOIN file_id_registries AS registry "
         "ON registry.registry_id=owner.registry_id"),
        ("declared_hash", "declared_hash_targets AS typed JOIN hash_values AS owner USING(hash_id)"),
        ("observed_content_hash", "observed_content_hash_targets AS typed JOIN hash_values AS owner USING(hash_id)"),
        ("unresolved_catalog", "unresolved_catalog_targets AS typed JOIN catalog_editions AS owner "
         "ON owner.edition_id=typed.edition_id"),
        ("external_record", "external_catalog_targets AS typed"),
    )
    matching = " OR ".join([actual_set, actual_media,
                            *(f"(target.target_kind={_lit(kind)} AND EXISTS(SELECT 1 FROM {joins} "
                              f"WHERE typed.target_id={target_id}))" for kind, joins in other_endpoints)])
    return (f"EXISTS(SELECT 1 FROM catalog_relationship_targets AS target "
            f"WHERE target.target_id={target_id} AND {typed_count} AND ({matching}))")


def _sealed(relationship_id):
    return ("(EXISTS(SELECT 1 FROM catalog_relationship_evidence_publications AS evidence "
            f"WHERE evidence.relationship_id={relationship_id}) OR EXISTS("
            "SELECT 1 FROM catalog_relationship_reviews AS review "
            "JOIN catalog_relationship_review_publications AS publication USING(review_id) "
            f"WHERE review.relationship_id={relationship_id} AND review.decision='accepted'))")


def _endpoint_invalid(alias, column, native_kinds):
    return f"NOT {_target_valid(f'{alias}.{column}', native_kinds)}"


def _invalid(native_kinds, identity="relationship"):
    payload = f"NOT {_payload_valid(identity)}"
    endpoints = []
    for table, origin in PAYLOAD_ENDPOINTS:
        for column in ("from_target_id", "to_target_id"):
            endpoints.append(
                f"({identity}.origin={_lit(origin)} AND EXISTS(SELECT 1 FROM {table} AS payload "
                f"WHERE payload.relationship_id={identity}.relationship_id "
                f"AND {_endpoint_invalid('payload', column, native_kinds)}))")
    return "(" + " OR ".join([payload, *endpoints]) + ")"


def _native_kinds(manifest):
    owners = tuple(manifest)
    set_kinds = tuple(sorted({owner.kind for owner in owners if owner.parent_table == "catalog_sets"}))
    media_kinds = tuple(sorted({owner.kind for owner in owners if owner.media}))
    if not set_kinds or not media_kinds:
        raise ValueError("relationship endpoint closure requires the assembled native-owner manifest")
    return set_kinds, media_kinds


def sql(manifest):
    """Independent corruption audit for evidence-sealed and accepted assertions."""
    native_kinds = _native_kinds(manifest)
    sealed = ("(EXISTS(SELECT 1 FROM catalog_relationship_evidence_publications AS evidence "
             "WHERE evidence.relationship_id=relationship.relationship_id) OR EXISTS("
             "SELECT 1 FROM catalog_relationship_reviews AS review "
             "JOIN catalog_relationship_review_publications AS publication USING(review_id) "
             "WHERE review.relationship_id=relationship.relationship_id AND review.decision='accepted'))")
    return (
        "CREATE VIEW candidate_relationship_closure_problems AS "
        "SELECT 'sealed_relationship_payload_or_endpoint' AS problem,relationship.relationship_id AS owner_id,relationship.edition_id "
        "FROM catalog_relationships AS relationship "
        "WHERE relationship.origin IN ('user','derived') AND " + sealed + " AND " + _invalid(native_kinds) + ";")


def audit_queries():
    return ["SELECT problem,owner_id,edition_id FROM candidate_relationship_closure_problems"]


def guards(manifest):
    """Close user/derived publication and freeze their meaning after sealing."""
    native_kinds = _native_kinds(manifest)
    result = []
    result.append(
        "CREATE TRIGGER relationship_closure_evidence_guard "
        "BEFORE INSERT ON catalog_relationship_evidence_publications "
        "WHEN EXISTS(SELECT 1 FROM catalog_relationships AS relationship "
        "WHERE relationship.relationship_id=NEW.relationship_id "
        "AND relationship.origin IN ('user','derived') AND " + _invalid(native_kinds) + ") "
        "BEGIN SELECT RAISE(ABORT,'relationship evidence requires exact payload and typed endpoints'); END;")
    result.append(
        "CREATE TRIGGER relationship_closure_review_guard "
        "BEFORE INSERT ON catalog_relationship_review_publications "
        "WHEN EXISTS(SELECT 1 FROM catalog_relationship_reviews AS review "
        "JOIN catalog_relationships AS relationship USING(relationship_id) "
        "WHERE review.review_id=NEW.review_id AND review.decision='accepted' "
        "AND relationship.origin IN ('user','derived') AND " + _invalid(native_kinds) + ") "
        "BEGIN SELECT RAISE(ABORT,'accepted relationship review requires exact payload and typed endpoints'); END;")

    # An UPDATE can move a draft child from one relationship onto a sealed one.
    for table in ("manual_catalog_relationships", "inferred_catalog_relationships",
                  "catalog_relationship_rationales", "catalog_relationship_comparisons",
                  "catalog_relationship_comparison_fields", "catalog_relationship_evidence"):
        for operation in ("INSERT", "UPDATE", "DELETE"):
            ids = (("NEW.relationship_id",) if operation == "INSERT" else
                   ("OLD.relationship_id", "NEW.relationship_id") if operation == "UPDATE" else
                   ("OLD.relationship_id",))
            condition = " OR ".join(_sealed(value) for value in ids)
            result.append(
                f"CREATE TRIGGER relationship_closure_{table}_{operation.lower()} "
                f"BEFORE {operation} ON {table} WHEN {condition} "
                "BEGIN SELECT RAISE(ABORT,'sealed relationship meaning is immutable'); END;")

    references = [f"EXISTS(SELECT 1 FROM {table} AS payload WHERE payload.{column}=TARGET_ID "
                  f"AND {_sealed('payload.relationship_id')})"
                  for table, _ in PAYLOAD_ENDPOINTS
                  for column in ("from_target_id", "to_target_id")]
    for table, _ in (("catalog_relationship_targets", "registry"), *ENDPOINTS):
        for operation in ("INSERT", "UPDATE", "DELETE"):
            ids = (("NEW.target_id",) if operation == "INSERT" else
                   ("OLD.target_id", "NEW.target_id") if operation == "UPDATE" else
                   ("OLD.target_id",))
            condition = " OR ".join(
                "(" + " OR ".join(reference.replace("TARGET_ID", target_id)
                                   for reference in references) + ")"
                for target_id in ids)
            result.append(
                f"CREATE TRIGGER relationship_closure_{table}_{operation.lower()} "
                f"BEFORE {operation} ON {table} WHEN {condition} "
                "BEGIN SELECT RAISE(ABORT,'sealed relationship endpoint is immutable'); END;")

    # Inferred assertions retain a rule identity, so its defining row is part
    # of their sealed meaning even though the relationship payload is frozen.
    for operation in ("UPDATE", "DELETE"):
        result.append(
            f"CREATE TRIGGER relationship_closure_catalog_relationship_rules_{operation.lower()} "
            f"BEFORE {operation} ON catalog_relationship_rules "
            "WHEN EXISTS(SELECT 1 FROM inferred_catalog_relationships AS inferred "
            "WHERE inferred.rule_id=OLD.rule_id AND "
            + _sealed("inferred.relationship_id") + ") "
            "BEGIN SELECT RAISE(ABORT,'sealed relationship meaning is immutable'); END;")

    sealed_unresolved_edition = " OR ".join(
        f"EXISTS(SELECT 1 FROM {table} AS payload "
        f"WHERE (payload.from_target_id=unresolved.target_id "
        f"OR payload.to_target_id=unresolved.target_id) AND "
        f"{_sealed('payload.relationship_id')})"
        for table in ("manual_catalog_relationships", "inferred_catalog_relationships"))
    for operation in ("UPDATE", "DELETE"):
        result.append(
            f"CREATE TRIGGER relationship_closure_catalog_editions_{operation.lower()} "
            "BEFORE " + operation + " ON catalog_editions "
            "WHEN EXISTS(SELECT 1 FROM unresolved_catalog_targets AS unresolved "
            "WHERE unresolved.edition_id=OLD.edition_id AND (" + sealed_unresolved_edition + ")) "
            "BEGIN SELECT RAISE(ABORT,'sealed relationship meaning is immutable'); END;")
    return result
