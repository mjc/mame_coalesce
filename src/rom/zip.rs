use crate::rayon::prelude::*;
use crate::zip::write::{ZipWriter, FileOptions};
use crate::rom;

use std::fs;
use std::path::{Path, PathBuf};

pub fn write_zip(bundle: &rom::Bundle, zip_dest: PathBuf) {
    let output_file_name = format!("{}.zip", bundle.name);
    println!("Writing {}", output_file_name);
    let path: PathBuf = [zip_dest.to_str().unwrap(), output_file_name.as_str()]
        .iter()
        .collect();
    let output = fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(output);
    bundle.files.iter().for_each(|(sha, _file)| {
        match bundle
            .matches
            .iter()
            .find(|(sha1, _dest, _src)| sha == sha1)
        {
            Some((_sha1, dest, src)) => {
                let mut source = fs::File::open(Path::new(src)).unwrap();
                zip.start_file(dest, FileOptions::default()).unwrap();
                std::io::copy(&mut source, &mut zip).unwrap();
            }
            None => (),
        }
    });
    zip.finish().unwrap();
}

pub fn write_all_zip(bundles: Vec<rom::Bundle>, zip_dest: &PathBuf) {
    bundles
        .par_iter()
        .for_each(|bundle| write_zip(bundle, zip_dest.to_path_buf()));
}
