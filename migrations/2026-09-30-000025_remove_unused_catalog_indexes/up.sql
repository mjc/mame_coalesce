-- Catalog reads are snapshot/set scoped, and MAME dependency reads use the set_id
-- primary key. Neither global string index is used by a production query.
DROP INDEX requirements_asset_name_index;
DROP INDEX mame_machine_dependencies_target_index;
