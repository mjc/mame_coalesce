-- MAME machine XML candidate. Depends on shared.sql and its declared interfaces.
-- Root facts are physical-root facts. Catalog-set identity/placement lives in
-- catalog_sets; ordinary child identity/order lives on each typed child owner.

CREATE TABLE mame_documents (
    edition_id INTEGER PRIMARY KEY REFERENCES catalog_editions(edition_id),
    build TEXT,
    debug INTEGER NOT NULL CHECK (debug IN (0,1)),
    debug_specified INTEGER NOT NULL CHECK (debug_specified IN (0,1)),
    mameconfig TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    extent_view TEXT CHECK (extent_view IS NULL OR extent_view IN ('retained_original_bytes','transport_decoded_xml_bytes')),
    extent_start INTEGER,
    extent_end INTEGER,
    location_view TEXT CHECK (location_view IS NULL OR location_view IN ('retained_original_text','transport_decoded_xml_text','decoded_dat_text')),
    start_line INTEGER,
    start_column INTEGER,
    end_line INTEGER,
    end_column INTEGER,
    column_convention TEXT CHECK (column_convention IS NULL OR column_convention='one_based_unicode_scalar'),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK (extent_start IS NULL OR (extent_start >= 0 AND extent_end > extent_start)),
    CHECK ((location_view IS NULL) = (start_line IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((start_column IS NULL) = (end_line IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK ((location_view IS NULL) = (column_convention IS NULL)),
    CHECK (start_line IS NULL OR (start_line > 0 AND start_column > 0 AND end_line > 0 AND end_column > 0)),
    CHECK (start_line IS NULL OR end_line > start_line OR (end_line = start_line AND end_column > start_column)),
    CHECK (extent_view IS NOT NULL OR location_view IS NOT NULL),
    CHECK (debug_specified=1 OR debug=0)
) STRICT, WITHOUT ROWID;

CREATE TABLE mame_machines (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_sets(set_id),
    sourcefile TEXT,
    isbios INTEGER NOT NULL CHECK (isbios IN (0,1)), isbios_specified INTEGER NOT NULL CHECK (isbios_specified IN (0,1)),
    isdevice INTEGER NOT NULL CHECK (isdevice IN (0,1)), isdevice_specified INTEGER NOT NULL CHECK (isdevice_specified IN (0,1)),
    ismechanical INTEGER NOT NULL CHECK (ismechanical IN (0,1)), ismechanical_specified INTEGER NOT NULL CHECK (ismechanical_specified IN (0,1)),
    runnable INTEGER NOT NULL CHECK (runnable IN (0,1)), runnable_specified INTEGER NOT NULL CHECK (runnable_specified IN (0,1)),
    CHECK (isbios_specified=1 OR isbios=0), CHECK (isdevice_specified=1 OR isdevice=0),
    CHECK (ismechanical_specified=1 OR ismechanical=0), CHECK (runnable_specified=1 OR runnable=1)
) STRICT, WITHOUT ROWID;

CREATE TABLE mame_machine_text_elements (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('description','year','manufacturer')),
    text_value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order>=0),
    source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0),
    UNIQUE(machine_id,field_kind), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_machine_text_elements_parent_order ON mame_machine_text_elements(machine_id,source_order);

CREATE TABLE mame_machine_compatibility (
    set_id INTEGER PRIMARY KEY REFERENCES mame_machines(set_id),
    isconsumable INTEGER NOT NULL CHECK (isconsumable IN (0,1))
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_links (
    machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    link_kind TEXT NOT NULL CHECK (link_kind IN ('cloneof','romof','sampleof')),
    target_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY(machine_id,link_kind)
) STRICT, WITHOUT ROWID;

CREATE TABLE mame_bios_sets (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    name TEXT NOT NULL, description TEXT NOT NULL,
    "default" INTEGER NOT NULL CHECK ("default" IN (0,1)), default_specified INTEGER NOT NULL CHECK (default_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0),
    UNIQUE(machine_id,source_order), CHECK (default_specified=1 OR "default"=0)
) STRICT;
CREATE INDEX mame_bios_sets_parent_order ON mame_bios_sets(machine_id,source_order);

CREATE TABLE mame_roms (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    name TEXT NOT NULL, size_text TEXT, bios TEXT, region TEXT, offset TEXT,
    status TEXT NOT NULL CHECK (status IN ('baddump','nodump','good')), status_specified INTEGER NOT NULL CHECK (status_specified IN (0,1)),
    optional INTEGER NOT NULL CHECK (optional IN (0,1)), optional_specified INTEGER NOT NULL CHECK (optional_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0),
    UNIQUE(machine_id,source_order), CHECK (status_specified=1 OR status='good'), CHECK (optional_specified=1 OR optional=0)
) STRICT;
CREATE INDEX mame_roms_parent_order ON mame_roms(machine_id,source_order);
CREATE TABLE mame_disks (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    name TEXT NOT NULL, region TEXT, "index" TEXT,
    writable INTEGER NOT NULL CHECK (writable IN (0,1)), writable_specified INTEGER NOT NULL CHECK (writable_specified IN (0,1)),
    status TEXT NOT NULL CHECK (status IN ('baddump','nodump','good')), status_specified INTEGER NOT NULL CHECK (status_specified IN (0,1)),
    optional INTEGER NOT NULL CHECK (optional IN (0,1)), optional_specified INTEGER NOT NULL CHECK (optional_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0),
    UNIQUE(machine_id,source_order), CHECK (writable_specified=1 OR writable=0), CHECK (status_specified=1 OR status='good'), CHECK (optional_specified=1 OR optional=0)
) STRICT;
CREATE INDEX mame_disks_parent_order ON mame_disks(machine_id,source_order);
CREATE TABLE mame_samples (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id), name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0),
    UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_samples_parent_order ON mame_samples(machine_id,source_order);
CREATE TABLE mame_rom_merges (
    media_entry_id INTEGER PRIMARY KEY REFERENCES mame_roms(media_entry_id),
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id), merge_name TEXT NOT NULL
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_disk_merges (
    media_entry_id INTEGER PRIMARY KEY REFERENCES mame_disks(media_entry_id),
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id), merge_name TEXT NOT NULL
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_device_references (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    tag TEXT NOT NULL, name TEXT NOT NULL, relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0),
    UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_device_references_parent_order ON mame_device_references(machine_id,source_order);

-- Hardware elements. All text that resembles a number remains exact source TEXT.
CREATE TABLE mame_chips (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    name TEXT NOT NULL, tag TEXT, type TEXT NOT NULL CHECK (type IN ('cpu','audio')), clock TEXT,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_chips_parent_order ON mame_chips(machine_id,source_order);
CREATE TABLE mame_displays (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    tag TEXT, type TEXT NOT NULL CHECK (type IN ('raster','vector','lcd','svg','unknown')), rotate TEXT CHECK (rotate IS NULL OR rotate IN ('0','90','180','270')),
    flipx INTEGER NOT NULL CHECK (flipx IN (0,1)), flipx_specified INTEGER NOT NULL CHECK (flipx_specified IN (0,1)),
    width TEXT, height TEXT, refresh TEXT NOT NULL, pixclock TEXT, htotal TEXT, hbend TEXT, hbstart TEXT, vtotal TEXT, vbend TEXT, vbstart TEXT,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0),
    UNIQUE(machine_id,source_order), CHECK (flipx_specified=1 OR flipx=0)
) STRICT;
CREATE INDEX mame_displays_parent_order ON mame_displays(machine_id,source_order);
CREATE TABLE mame_sound (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL UNIQUE REFERENCES mame_machines(set_id),
    channels TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE TABLE mame_inputs (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL UNIQUE REFERENCES mame_machines(set_id),
    service INTEGER NOT NULL CHECK (service IN (0,1)), service_specified INTEGER NOT NULL CHECK (service_specified IN (0,1)),
    tilt INTEGER NOT NULL CHECK (tilt IN (0,1)), tilt_specified INTEGER NOT NULL CHECK (tilt_specified IN (0,1)), players TEXT NOT NULL, coins TEXT,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order),
    CHECK (service_specified=1 OR service=0), CHECK (tilt_specified=1 OR tilt=0)
) STRICT;
CREATE TABLE mame_input_controls (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), input_id INTEGER NOT NULL REFERENCES mame_inputs(source_element_id),
    type TEXT NOT NULL, player TEXT, buttons TEXT, minimum TEXT, maximum TEXT, sensitivity TEXT, keydelta TEXT,
    reverse INTEGER NOT NULL CHECK (reverse IN (0,1)), reverse_specified INTEGER NOT NULL CHECK (reverse_specified IN (0,1)), ways TEXT, ways2 TEXT, ways3 TEXT,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(input_id,source_order),
    CHECK (reverse_specified=1 OR reverse=0)
) STRICT;
CREATE INDEX mame_input_controls_parent_order ON mame_input_controls(input_id,source_order);
CREATE TABLE mame_switches (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    kind TEXT NOT NULL CHECK (kind IN ('dipswitch','configuration')), name TEXT NOT NULL, tag TEXT NOT NULL, mask TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_switches_parent_order ON mame_switches(machine_id,source_order);
CREATE TABLE mame_switch_locations (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), switch_id INTEGER NOT NULL REFERENCES mame_switches(source_element_id),
    name TEXT NOT NULL, number TEXT NOT NULL, inverted INTEGER NOT NULL CHECK (inverted IN (0,1)), inverted_specified INTEGER NOT NULL CHECK (inverted_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(switch_id,source_order),
    CHECK (inverted_specified=1 OR inverted=0)
) STRICT;
CREATE INDEX mame_switch_locations_parent_order ON mame_switch_locations(switch_id,source_order);
CREATE TABLE mame_switch_values (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), switch_id INTEGER NOT NULL REFERENCES mame_switches(source_element_id),
    name TEXT NOT NULL, value TEXT NOT NULL, "default" INTEGER NOT NULL CHECK ("default" IN (0,1)), default_specified INTEGER NOT NULL CHECK (default_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(switch_id,source_order),
    CHECK (default_specified=1 OR "default"=0)
) STRICT;
CREATE INDEX mame_switch_values_parent_order ON mame_switch_values(switch_id,source_order);
CREATE TABLE mame_switch_conditions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), switch_id INTEGER NOT NULL UNIQUE REFERENCES mame_switches(source_element_id),
    tag TEXT NOT NULL, mask TEXT NOT NULL, relation TEXT NOT NULL CHECK (relation IN ('eq','ne','gt','le','lt','ge')), value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0)
) STRICT;
CREATE TABLE mame_switch_value_conditions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), value_id INTEGER NOT NULL UNIQUE REFERENCES mame_switch_values(source_element_id),
    tag TEXT NOT NULL, mask TEXT NOT NULL, relation TEXT NOT NULL CHECK (relation IN ('eq','ne','gt','le','lt','ge')), value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0)
) STRICT;
CREATE TABLE mame_adjusters (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    name TEXT NOT NULL, "default" TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_adjusters_parent_order ON mame_adjusters(machine_id,source_order);
CREATE TABLE mame_adjuster_conditions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), adjuster_id INTEGER NOT NULL UNIQUE REFERENCES mame_adjusters(source_element_id),
    tag TEXT NOT NULL, mask TEXT NOT NULL, relation TEXT NOT NULL CHECK (relation IN ('eq','ne','gt','le','lt','ge')), value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0)
) STRICT;
CREATE TABLE mame_ports (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    tag TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_ports_parent_order ON mame_ports(machine_id,source_order);
CREATE TABLE mame_analogs (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), port_id INTEGER NOT NULL REFERENCES mame_ports(source_element_id),
    mask TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(port_id,source_order)
) STRICT;
CREATE INDEX mame_analogs_parent_order ON mame_analogs(port_id,source_order);
CREATE TABLE mame_drivers (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL UNIQUE REFERENCES mame_machines(set_id),
    status TEXT NOT NULL CHECK (status IN ('good','imperfect','preliminary')), emulation TEXT NOT NULL CHECK (emulation IN ('good','imperfect','preliminary')),
    cocktail TEXT CHECK (cocktail IS NULL OR cocktail IN ('good','imperfect','preliminary')), savestate TEXT NOT NULL CHECK (savestate IN ('supported','unsupported')),
    requiresartwork INTEGER NOT NULL CHECK (requiresartwork IN (0,1)), requiresartwork_specified INTEGER NOT NULL CHECK (requiresartwork_specified IN (0,1)),
    unofficial INTEGER NOT NULL CHECK (unofficial IN (0,1)), unofficial_specified INTEGER NOT NULL CHECK (unofficial_specified IN (0,1)),
    nosoundhardware INTEGER NOT NULL CHECK (nosoundhardware IN (0,1)), nosoundhardware_specified INTEGER NOT NULL CHECK (nosoundhardware_specified IN (0,1)),
    incomplete INTEGER NOT NULL CHECK (incomplete IN (0,1)), incomplete_specified INTEGER NOT NULL CHECK (incomplete_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order),
    CHECK (requiresartwork_specified=1 OR requiresartwork=0), CHECK (unofficial_specified=1 OR unofficial=0),
    CHECK (nosoundhardware_specified=1 OR nosoundhardware=0), CHECK (incomplete_specified=1 OR incomplete=0)
) STRICT;
CREATE TABLE mame_features (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    type TEXT NOT NULL CHECK (type IN ('protection','timing','graphics','palette','sound','capture','camera','microphone','controls','keyboard','mouse','media','disk','printer','tape','punch','drum','rom','comms','lan','wan')),
    status TEXT CHECK (status IS NULL OR status IN ('unemulated','imperfect')), overall TEXT CHECK (overall IS NULL OR overall IN ('unemulated','imperfect')),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_features_parent_order ON mame_features(machine_id,source_order);
CREATE TABLE mame_devices (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    type TEXT NOT NULL, tag TEXT, fixed_image TEXT, mandatory TEXT, interface TEXT,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_devices_parent_order ON mame_devices(machine_id,source_order);
CREATE TABLE mame_device_instances (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), device_id INTEGER NOT NULL REFERENCES mame_devices(source_element_id),
    name TEXT NOT NULL, briefname TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(device_id,source_order)
) STRICT;
CREATE INDEX mame_device_instances_parent_order ON mame_device_instances(device_id,source_order);
CREATE TABLE mame_device_extensions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), device_id INTEGER NOT NULL REFERENCES mame_devices(source_element_id),
    name TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(device_id,source_order)
) STRICT;
CREATE INDEX mame_device_extensions_parent_order ON mame_device_extensions(device_id,source_order);
CREATE TABLE mame_slots (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    name TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_slots_parent_order ON mame_slots(machine_id,source_order);
CREATE TABLE mame_slot_options (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), slot_id INTEGER NOT NULL REFERENCES mame_slots(source_element_id),
    name TEXT NOT NULL, devname TEXT NOT NULL, "default" INTEGER NOT NULL CHECK ("default" IN (0,1)), default_specified INTEGER NOT NULL CHECK (default_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(slot_id,source_order),
    CHECK (default_specified=1 OR "default"=0)
) STRICT;
CREATE INDEX mame_slot_options_parent_order ON mame_slot_options(slot_id,source_order);
CREATE TABLE mame_softwarelist_references (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    tag TEXT NOT NULL, name TEXT NOT NULL, status TEXT NOT NULL CHECK (status IN ('original','compatible')), filter TEXT,
    source_order INTEGER NOT NULL CHECK (source_order>=0), source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_softwarelist_references_parent_order ON mame_softwarelist_references(machine_id,source_order);
CREATE TABLE mame_ram_options (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id), machine_id INTEGER NOT NULL REFERENCES mame_machines(set_id),
    name TEXT NOT NULL, "default" TEXT, text TEXT NOT NULL, source_order INTEGER NOT NULL CHECK (source_order>=0),
    source_line INTEGER NOT NULL CHECK (source_line>0), source_column INTEGER NOT NULL CHECK (source_column>0), UNIQUE(machine_id,source_order)
) STRICT;
CREATE INDEX mame_ram_options_parent_order ON mame_ram_options(machine_id,source_order);

CREATE TABLE mame_rom_compatibility (
    media_entry_id INTEGER PRIMARY KEY REFERENCES mame_roms(media_entry_id),
    soundonly INTEGER CHECK (soundonly IS NULL OR soundonly IN (0,1)), dispose INTEGER CHECK (dispose IS NULL OR dispose IN (0,1)),
    loadflag TEXT, value TEXT, inverted INTEGER CHECK (inverted IS NULL OR inverted IN (0,1)), ovha TEXT, nothread INTEGER CHECK (nothread IS NULL OR nothread IN (0,1)),
    CHECK (soundonly IS NOT NULL OR dispose IS NOT NULL OR loadflag IS NOT NULL OR value IS NOT NULL OR inverted IS NOT NULL OR ovha IS NOT NULL OR nothread IS NOT NULL)
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_disk_compatibility (
    media_entry_id INTEGER PRIMARY KEY REFERENCES mame_disks(media_entry_id),
    writeable INTEGER NOT NULL CHECK (writeable IN (0,1))
) STRICT, WITHOUT ROWID;

-- Position ledgers are closed text-code domains. Every accepted singleton has
-- field_occurrence=0. Hash positions alone carry reported_hash_id. Each table's
-- UNIQUE(source_order) combines local code uniqueness with one tag ordinal;
-- the assembler supplies cross-table ordinal, presence, kind and edition guards.
CREATE TABLE mame_document_facts_attribute_positions (
    edition_id INTEGER NOT NULL REFERENCES mame_documents(edition_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('build','debug','mameconfig')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0),
    source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(edition_id,field_kind,field_occurrence), UNIQUE(edition_id,source_order)
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machines_attribute_positions (
    set_id INTEGER NOT NULL REFERENCES mame_machines(set_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','sourcefile','isbios','isdevice','ismechanical','runnable','cloneof','romof','sampleof')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    source_order INTEGER NOT NULL CHECK(source_order>=0),
    source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(set_id,field_kind,field_occurrence), UNIQUE(set_id,source_order),
    CHECK ((field_kind IN ('cloneof','romof','sampleof')) = (relationship_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_compatibility_attribute_positions (
    set_id INTEGER NOT NULL REFERENCES mame_machines(set_id), field_kind TEXT NOT NULL CHECK(field_kind='isconsumable'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0),
    source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(set_id,field_kind,field_occurrence), UNIQUE(set_id,source_order)
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_bios_sets_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES mame_bios_sets(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','description','default')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_rom_claims_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES mame_roms(media_entry_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','bios','size','crc','sha1','merge','region','offset','status','optional')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), reported_hash_id INTEGER REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(media_entry_id,field_kind,field_occurrence), UNIQUE(media_entry_id,source_order), UNIQUE(reported_hash_id),
    CHECK ((field_kind IN ('crc','sha1')) = (reported_hash_id IS NOT NULL)),
    CHECK ((field_kind='merge') = (relationship_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_rom_compatibility_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES mame_roms(media_entry_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('md5','soundonly','dispose','loadflag','value','inverted','ovha','nothread')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), reported_hash_id INTEGER REFERENCES catalog_entry_hashes(reported_hash_id),
    source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(media_entry_id,field_kind,field_occurrence), UNIQUE(media_entry_id,source_order), UNIQUE(reported_hash_id),
    CHECK ((field_kind='md5') = (reported_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_disk_claims_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES mame_disks(media_entry_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','sha1','merge','region','index','writable','status','optional')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), reported_hash_id INTEGER REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(media_entry_id,field_kind,field_occurrence), UNIQUE(media_entry_id,source_order), UNIQUE(reported_hash_id),
    CHECK ((field_kind='sha1') = (reported_hash_id IS NOT NULL)),
    CHECK ((field_kind='merge') = (relationship_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_disk_compatibility_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES mame_disks(media_entry_id), field_kind TEXT NOT NULL CHECK(field_kind='writeable'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0),
    source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(media_entry_id,field_kind,field_occurrence), UNIQUE(media_entry_id,source_order)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER mame_rom_hash_position_owner_insert
BEFORE INSERT ON mame_rom_claims_attribute_positions
WHEN NEW.field_kind IN ('crc','sha1') AND NOT EXISTS (
    SELECT 1 FROM catalog_entry_hashes AS hash
    WHERE hash.reported_hash_id=NEW.reported_hash_id
      AND hash.media_entry_id=NEW.media_entry_id
      AND hash.source_hash_field=NEW.field_kind
      AND hash.field_occurrence=NEW.field_occurrence
)
BEGIN SELECT RAISE(ABORT,'MAME ROM hash position must reference its matching declaration'); END;
CREATE TRIGGER mame_rom_hash_position_owner_update
BEFORE UPDATE ON mame_rom_claims_attribute_positions
WHEN NEW.field_kind IN ('crc','sha1') AND NOT EXISTS (
    SELECT 1 FROM catalog_entry_hashes AS hash
    WHERE hash.reported_hash_id=NEW.reported_hash_id
      AND hash.media_entry_id=NEW.media_entry_id
      AND hash.source_hash_field=NEW.field_kind
      AND hash.field_occurrence=NEW.field_occurrence
)
BEGIN SELECT RAISE(ABORT,'MAME ROM hash position must reference its matching declaration'); END;
CREATE TRIGGER mame_rom_compat_hash_position_owner_insert
BEFORE INSERT ON mame_rom_compatibility_attribute_positions
WHEN NEW.field_kind='md5' AND NOT EXISTS (
    SELECT 1 FROM catalog_entry_hashes AS hash
    WHERE hash.reported_hash_id=NEW.reported_hash_id
      AND hash.media_entry_id=NEW.media_entry_id
      AND hash.source_hash_field='md5'
      AND hash.field_occurrence=NEW.field_occurrence
)
BEGIN SELECT RAISE(ABORT,'MAME ROM MD5 position must reference its matching declaration'); END;
CREATE TRIGGER mame_rom_compat_hash_position_owner_update
BEFORE UPDATE ON mame_rom_compatibility_attribute_positions
WHEN NEW.field_kind='md5' AND NOT EXISTS (
    SELECT 1 FROM catalog_entry_hashes AS hash
    WHERE hash.reported_hash_id=NEW.reported_hash_id
      AND hash.media_entry_id=NEW.media_entry_id
      AND hash.source_hash_field='md5'
      AND hash.field_occurrence=NEW.field_occurrence
)
BEGIN SELECT RAISE(ABORT,'MAME ROM MD5 position must reference its matching declaration'); END;
CREATE TRIGGER mame_disk_hash_position_owner_insert
BEFORE INSERT ON mame_disk_claims_attribute_positions
WHEN NEW.field_kind='sha1' AND NOT EXISTS (
    SELECT 1 FROM catalog_entry_hashes AS hash
    WHERE hash.reported_hash_id=NEW.reported_hash_id
      AND hash.media_entry_id=NEW.media_entry_id
      AND hash.source_hash_field='sha1'
      AND hash.field_occurrence=NEW.field_occurrence
)
BEGIN SELECT RAISE(ABORT,'MAME disk SHA-1 position must reference its matching declaration'); END;
CREATE TRIGGER mame_disk_hash_position_owner_update
BEFORE UPDATE ON mame_disk_claims_attribute_positions
WHEN NEW.field_kind='sha1' AND NOT EXISTS (
    SELECT 1 FROM catalog_entry_hashes AS hash
    WHERE hash.reported_hash_id=NEW.reported_hash_id
      AND hash.media_entry_id=NEW.media_entry_id
      AND hash.source_hash_field='sha1'
      AND hash.field_occurrence=NEW.field_occurrence
)
BEGIN SELECT RAISE(ABORT,'MAME disk SHA-1 position must reference its matching declaration'); END;
CREATE TABLE mame_device_references_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES mame_device_references(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('tag','name')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order),
    CHECK ((field_kind='name') = (relationship_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE TABLE mame_samples_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES mame_samples(media_entry_id), field_kind TEXT NOT NULL CHECK(field_kind='name'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0),
    source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
    PRIMARY KEY(media_entry_id,field_kind,field_occurrence), UNIQUE(media_entry_id,source_order)
) STRICT, WITHOUT ROWID;

-- Remaining native hardware position tables; owner is the actual element ID.
CREATE TABLE mame_machine_chips_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_chips(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','tag','type','clock')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_displays_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_displays(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('tag','type','rotate','flipx','width','height','refresh','pixclock','htotal','hbend','hbstart','vtotal','vbend','vbstart')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_sounds_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_sound(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind='channels'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_inputs_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_inputs(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('service','tilt','players','coins')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_input_controls_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_input_controls(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('type','player','buttons','minimum','maximum','sensitivity','keydelta','reverse','ways','ways2','ways3')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE machine_switches_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_switches(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','tag','mask')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE machine_switch_locations_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_switch_locations(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','number','inverted')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE machine_switch_values_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_switch_values(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','value','default')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE machine_switch_conditions_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_switch_conditions(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('tag','mask','relation','value')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE machine_switch_value_conditions_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_switch_value_conditions(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('tag','mask','relation','value')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_adjusters_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_adjusters(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','default')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_adjuster_conditions_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_adjuster_conditions(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('tag','mask','relation','value')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_ports_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_ports(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind='tag'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_analogs_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_analogs(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind='mask'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_drivers_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_drivers(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('status','emulation','cocktail','savestate','requiresartwork','unofficial','nosoundhardware','incomplete')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_features_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_features(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('type','status','overall')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_devices_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_devices(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('type','tag','fixed_image','mandatory','interface')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_device_instances_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_device_instances(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','briefname')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_device_extensions_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_device_extensions(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind='name'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_slots_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_slots(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind='name'), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_slot_options_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_slot_options(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','devname','default')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_software_lists_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_softwarelist_references(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('tag','name','status','filter')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;
CREATE TABLE mame_machine_ram_options_attribute_positions (source_element_id INTEGER NOT NULL REFERENCES mame_ram_options(source_element_id), field_kind TEXT NOT NULL CHECK(field_kind IN ('name','default')), field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK(field_occurrence=0), source_order INTEGER NOT NULL CHECK(source_order>=0), source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0), PRIMARY KEY(source_element_id,field_kind,field_occurrence), UNIQUE(source_element_id,source_order)) STRICT, WITHOUT ROWID;

-- This is a publication diagnostic, not an unconditional device_id UNIQUE:
-- the observed-v3 reading rule retains every instance, while strict DTD
-- rejects a second one. Unknown/legacy dialects are not silently classified
-- as strict.
CREATE VIEW candidate_mame_integrity_problems AS
SELECT 'strict_device_instance_cardinality' AS problem,
       device.source_element_id AS owner_id,
       edition.edition_id AS edition_id
FROM mame_devices AS device
JOIN catalog_source_elements AS element
  ON element.source_element_id=device.source_element_id
JOIN catalog_editions AS edition
  ON edition.edition_id=element.edition_id
JOIN catalog_reading_rules AS rules
  ON rules.reading_rules_id=edition.reading_rules_id
LEFT JOIN mame_device_instances AS instance
  ON instance.device_id=device.source_element_id
WHERE rules.format_family='mame'
  AND rules.dialect='strict-dtd'
GROUP BY device.source_element_id,edition.edition_id
HAVING count(instance.source_element_id)>1;
