use crate::rayon::prelude::*;
use crate::rom;
use crate::zip::write::{FileOptions, ZipWriter};

use std::fs;
use std::path::{Path, PathBuf};

pub fn write_zip(bundle: &rom::Bundle, zip_dest: PathBuf) {
    let output_file_name = format!("{}.zip", bundle.name);
    println!("Writing {}", output_file_name);
    let path: PathBuf = [zip_dest.to_str().unwrap(), output_file_name.as_str()]
        .iter()
        .collect();

    let found: Vec<(String, PathBuf)> = bundle
        .files
        .iter()
        .filter_map(|(sha, _file)| {
            match bundle
                .matches
                .iter()
                .find(|(sha1, _dest, _src)| sha == sha1)
            {
                Some((_sha1, dest, src)) => Some((dest.to_string(), src.to_path_buf())),
                None => None,
            }
        })
        .collect();
    if !found.is_empty() {
        let output =
            fs::File::create(&path).unwrap_or_else(|_| panic!("Couldn't create {:?}", &path));
        let mut zip = ZipWriter::new(output);
        found.iter().for_each(|(dest, src)| {
            let mut source =
                fs::File::open(Path::new(src)).unwrap_or_else(|_| panic!("Couldn't open {:?}", src));
            zip.start_file(dest, FileOptions::default())
                .unwrap_or_else(|_| panic!("Couldn't start zip: {:?}", dest));
            std::io::copy(&mut source, &mut zip)
                .unwrap_or_else(|_| panic!("Couldn't copy into zip: {:?}", src));
        });
        zip.finish()
            .unwrap_or_else(|_| panic!("Unable to finish zipfile: {:?}", output_file_name));
    }
}

pub fn write_all_zip(bundles: Vec<rom::Bundle>, zip_dest: &PathBuf) {
    bundles
        .par_iter()
        .for_each(|bundle| write_zip(bundle, zip_dest.to_path_buf()));
}
