"""Query-only numeric interpretation of native software-list declarations."""

from numeric_sql import software_integer_sql


def sql():
    """Use the pinned software grammar without duplicating retained source text.

    These are numeric projections, not proofs of a valid load recipe, complete
    file coverage, or shared-file identity. Invalid text remains on its native
    owner and projects NULL; zero is a valid number even where an operation
    requires a positive length.
    """
    return f"""
CREATE VIEW candidate_software_data_area_numbers AS
SELECT area.area_id,
       {software_integer_sql('area.declared_size_text')} AS byte_length
FROM software_data_areas AS area;

CREATE VIEW candidate_software_rom_numbers AS
SELECT rom.media_entry_id,
       {software_integer_sql('rom.size_text')} AS byte_length,
       {software_integer_sql('rom.offset_text')} AS byte_offset
FROM software_rom_load_entries AS rom;
""".strip()
