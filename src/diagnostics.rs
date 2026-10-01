//! Exact byte excerpts and highlights for import warnings and errors.

/// Provenance of line/column coordinates, independent of byte evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinateConvention {
    /// Decoded XML text: XML line-ending rules and one-based Unicode scalars.
    XmlUnicodeScalars,
    /// UTF-8 DAT text: lexer line endings and one-based Unicode scalars.
    DatUnicodeScalars,
}

impl CoordinateConvention {
    /// Stable coordinate-view storage identifier.
    #[must_use]
    pub const fn view(self) -> &'static str {
        match self {
            Self::XmlUnicodeScalars => "transport_decoded_xml_text",
            Self::DatUnicodeScalars => "decoded_dat_text",
        }
    }

    /// One-based column convention, not byte offsets or display columns.
    #[must_use]
    pub const fn columns(self) -> &'static str {
        "unicode_scalar_1based"
    }
}

/// A start-inclusive, end-exclusive byte range with checked ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteRange {
    start: usize,
    end: usize,
}

impl ByteRange {
    /// Construct a range, rejecting reversed boundaries.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Option<Self> {
        if start <= end {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Inclusive byte offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Exclusive byte offset; equal to `start` for an insertion/EOF anchor.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }
}

/// The reproducible byte stream from which an excerpt was captured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExcerptView {
    /// Bytes in the external, unchanged source document.
    RetainedOriginalBytes,
    /// XML bytes after gzip transport decoding, before text decoding/recovery.
    TransportDecodedXmlBytes,
}

impl ExcerptView {
    /// Stable storage identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RetainedOriginalBytes => "retained_original_bytes",
            Self::TransportDecodedXmlBytes => "transport_decoded_xml_bytes",
        }
    }
}

/// An exact source window, not normalized XML or a copy of a parsed record.
///
/// The complete offending range in the named view remains independent of the
/// window. A highlight is available only if that whole range is retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceExcerpt {
    bytes: Vec<u8>,
    view: ExcerptView,
    start_byte: usize,
    source_problem: Option<ByteRange>,
}

impl SourceExcerpt {
    /// Capture exact bytes. Invalid context or source ranges are rejected.
    ///
    /// `context` and `problem` are offsets in `source`, not offsets in the saved
    /// excerpt. Unknown problems are represented by `None`, never a guessed range.
    #[must_use]
    pub fn capture(
        source: &[u8],
        view: ExcerptView,
        context: ByteRange,
        problem: Option<ByteRange>,
    ) -> Option<Self> {
        if problem.is_some_and(|range| range.end > source.len()) {
            return None;
        }
        let bytes = source.get(context.start..context.end)?.to_vec();
        Some(Self {
            bytes,
            view,
            start_byte: context.start,
            source_problem: problem,
        })
    }

    /// Trim to an excerpt-local window, rebasing its anchor and any highlight.
    ///
    /// Clipping part of the problem makes the highlight unknown while retaining
    /// the complete source-view range. Empty EOF anchors are valid.
    #[must_use]
    pub fn clip(self, window: ByteRange) -> Option<Self> {
        let bytes = self.bytes.get(window.start..window.end)?.to_vec();
        Some(Self {
            bytes,
            start_byte: self.start_byte.checked_add(window.start)?,
            ..self
        })
    }

    /// The exact bytes retained in SQLite.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Which reproducible byte view owns the coordinates.
    #[must_use]
    pub const fn view(&self) -> ExcerptView {
        self.view
    }

    /// Excerpt start in that view.
    #[must_use]
    pub const fn start_byte(&self) -> usize {
        self.start_byte
    }

    /// Full offending range in the named view, independent of clipping.
    #[must_use]
    pub const fn source_problem(&self) -> Option<ByteRange> {
        self.source_problem
    }

    /// Highlight range relative to the bytes actually saved in this excerpt.
    #[must_use]
    pub fn problem(&self) -> Option<ByteRange> {
        let range = self.source_problem?;
        let start = range.start.checked_sub(self.start_byte)?;
        let end = range.end.checked_sub(self.start_byte)?;
        (end <= self.bytes.len()).then_some(ByteRange { start, end })
    }

    /// Exact retained-file range; decoded gzip bytes do not establish one.
    #[must_use]
    pub const fn original_problem(&self) -> Option<ByteRange> {
        match self.view {
            ExcerptView::RetainedOriginalBytes => self.source_problem,
            ExcerptView::TransportDecodedXmlBytes => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn range(start: usize, end: usize) -> ByteRange {
        ByteRange::new(start, end).expect("ordered range")
    }

    #[test]
    fn prefix_clipping_rebases_the_highlight_and_retains_the_full_source_span() {
        let excerpt = SourceExcerpt::capture(
            b"0123456789abcdef",
            ExcerptView::RetainedOriginalBytes,
            range(0, 16),
            Some(range(10, 12)),
        )
        .expect("valid source window");
        let clipped = excerpt.clip(range(8, 16)).expect("valid clip");
        assert_eq!(clipped.bytes(), b"89abcdef");
        assert_eq!(clipped.start_byte(), 8);
        assert_eq!(clipped.problem(), Some(range(2, 4)));
        assert_eq!(clipped.source_problem(), Some(range(10, 12)));
        assert_eq!(clipped.original_problem(), Some(range(10, 12)));
    }

    #[test]
    fn partial_problem_is_not_a_highlight_and_gzip_is_not_an_original_offset() {
        let excerpt = SourceExcerpt::capture(
            b"0123456789abcdef",
            ExcerptView::TransportDecodedXmlBytes,
            range(0, 16),
            Some(range(10, 12)),
        )
        .expect("valid window");
        let clipped = excerpt.clip(range(11, 16)).expect("valid clip");
        assert_eq!(clipped.problem(), None);
        assert_eq!(clipped.source_problem(), Some(range(10, 12)));
        assert_eq!(clipped.original_problem(), None);
    }

    #[test]
    fn empty_eof_anchors_and_unknown_ranges_are_distinct() {
        let excerpt = SourceExcerpt::capture(
            b"\0\xff",
            ExcerptView::RetainedOriginalBytes,
            range(0, 2),
            Some(range(2, 2)),
        )
        .expect("EOF anchor");
        assert_eq!(excerpt.problem(), Some(range(2, 2)));
        let unknown = SourceExcerpt::capture(
            b"\0\xff",
            ExcerptView::RetainedOriginalBytes,
            range(0, 2),
            None,
        )
        .expect("unknown highlight");
        assert_eq!(unknown.problem(), None);
        assert_eq!(unknown.bytes(), b"\0\xff");
        assert_eq!(ByteRange::new(2, 1), None);
        assert!(
            SourceExcerpt::capture(b"x", ExcerptView::RetainedOriginalBytes, range(0, 2), None)
                .is_none()
        );
        assert!(
            SourceExcerpt::capture(
                b"x",
                ExcerptView::RetainedOriginalBytes,
                range(0, 1),
                Some(range(0, 2))
            )
            .is_none()
        );
    }
}
