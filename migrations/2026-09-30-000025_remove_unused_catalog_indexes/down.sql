CREATE INDEX IF NOT EXISTS requirements_asset_name_index ON asset_requirements (asset_name);
CREATE INDEX IF NOT EXISTS mame_machine_dependencies_target_index
    ON mame_machine_dependencies (target_name, dependency_kind, set_id);
