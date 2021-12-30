CREATE INDEX game_name ON games (
    name
);

CREATE INDEX md5_index ON roms (
    md5
);

CREATE INDEX sha1_index ON roms (
    sha1
);

CREATE INDEX crc_index ON roms (
    crc
);
