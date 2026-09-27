DROP INDEX snapshot_extensions_asset_owner_index;

ALTER TABLE snapshot_extensions DROP COLUMN owner_component_order;
ALTER TABLE snapshot_extensions DROP COLUMN owner_set_name;
