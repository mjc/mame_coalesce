use std::{borrow::Cow, io::Read};

use flate2::read::MultiGzDecoder;

pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_DECOMPRESSED_DOCUMENT_BYTES: usize = 128 * 1024 * 1024;

pub fn read_bounded<R: Read>(reader: R, limit: usize) -> crate::Result<Vec<u8>> {
    let read_limit = limit
        .checked_add(1)
        .ok_or(crate::Error::DocumentTooLarge { limit })?;
    let mut bytes = Vec::with_capacity(read_limit.min(8192));
    reader
        .take(u64::try_from(read_limit).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(crate::Error::DocumentTooLarge { limit });
    }
    Ok(bytes)
}

pub fn decode_xml(bytes: &[u8]) -> crate::Result<Cow<'_, [u8]>> {
    decode_xml_with_limit(bytes, MAX_DECOMPRESSED_DOCUMENT_BYTES)
}

fn decode_xml_with_limit(bytes: &[u8], limit: usize) -> crate::Result<Cow<'_, [u8]>> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let decoder = MultiGzDecoder::new(bytes);
        Ok(Cow::Owned(read_bounded(decoder, limit)?))
    } else {
        Ok(Cow::Borrowed(bytes))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{Compression, write::GzEncoder};

    use super::*;

    #[test]
    fn gzip_decoder_rejects_expansion_past_limit() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        assert!(encoder.write_all(b"document larger than cap").is_ok());
        let compressed = encoder.finish();
        assert!(compressed.is_ok());
        let compressed = compressed.unwrap_or_default();
        assert!(matches!(
            decode_xml_with_limit(&compressed, 8),
            Err(crate::Error::DocumentTooLarge { limit: 8 })
        ));
    }

    #[test]
    fn retained_document_limit_rejects_the_first_excess_byte() {
        assert_eq!(MAX_DOCUMENT_BYTES, 64 * 1024 * 1024);
        let input = std::io::repeat(0).take((MAX_DOCUMENT_BYTES + 1) as u64);
        assert!(matches!(
            read_bounded(input, MAX_DOCUMENT_BYTES),
            Err(crate::Error::DocumentTooLarge {
                limit: MAX_DOCUMENT_BYTES
            })
        ));
    }

    #[test]
    fn malformed_gzip_is_a_structured_input_error() {
        assert!(matches!(
            decode_xml(&[0x1f, 0x8b, 0x00]),
            Err(crate::Error::Io(_))
        ));
    }
}
