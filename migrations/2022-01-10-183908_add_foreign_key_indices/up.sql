-- sqlite does not automatically index foreign keys

CREATE INDEX rom_file_rom_id_relation_index ON rom_files (
    rom_id
);


CREATE INDEX roms_game_id_relation_index ON roms (
    game_id
);

CREATE INDEX games_parent_id_relation_index ON games (
    parent_id
);

CREATE INDEX games_data_file_id_relation_index ON games (
    data_file_id
);
