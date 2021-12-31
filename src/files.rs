pub struct RomFile {
    path: String,
    name: String,
    crc: Vec<u8>,
    sha1: Vec<u8>,
    md5: Vec<u8>,
    in_archive: bool,
}
