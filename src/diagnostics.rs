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

/// An exact source window and the byte evidence known about it.
///
/// Each range is independently nullable to preserve the database contract. A
/// local highlight addresses the saved bytes; source and original ranges
/// address their named byte streams. Missing evidence is never inferred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceExcerpt {
    bytes: Vec<u8>,
    view: ExcerptView,
    start_byte: Option<usize>,
    problem: Option<ByteRange>,
    source_problem: Option<ByteRange>,
    original_problem: Option<ByteRange>,
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
        let local_problem = problem.and_then(|range| {
            let start = range.start.checked_sub(context.start)?;
            let end = range.end.checked_sub(context.start)?;
            (range.end <= context.end).then_some(ByteRange { start, end })
        });
        let original_problem = match view {
            ExcerptView::RetainedOriginalBytes => problem,
            ExcerptView::TransportDecodedXmlBytes => None,
        };
        Self::from_saved_parts(
            bytes,
            view,
            Some(context.start),
            local_problem,
            problem,
            original_problem,
        )
    }

    /// Preserve byte evidence read from storage, rejecting contradictions.
    ///
    /// A missing anchor or range remains missing. When the anchor and local
    /// highlight are both known, their checked sum must match each known full
    /// range. Full ranges may extend beyond the saved excerpt after clipping.
    /// An original-file range is valid only for retained original bytes.
    #[must_use]
    pub fn from_saved_parts(
        bytes: Vec<u8>,
        view: ExcerptView,
        start_byte: Option<usize>,
        problem: Option<ByteRange>,
        source_problem: Option<ByteRange>,
        original_problem: Option<ByteRange>,
    ) -> Option<Self> {
        if problem.is_some_and(|range| range.end > bytes.len())
            || (original_problem.is_some() && view != ExcerptView::RetainedOriginalBytes)
            || matches!(
                (source_problem, original_problem),
                (Some(source), Some(original)) if source != original
            )
        {
            return None;
        }

        if let (Some(anchor), Some(local)) = (start_byte, problem) {
            let expected = ByteRange {
                start: anchor.checked_add(local.start)?,
                end: anchor.checked_add(local.end)?,
            };
            if source_problem.is_some_and(|source| source != expected)
                || original_problem.is_some_and(|original| original != expected)
            {
                return None;
            }
        }

        Some(Self {
            bytes,
            view,
            start_byte,
            problem,
            source_problem,
            original_problem,
        })
    }

    /// Trim to an excerpt-local window, rebasing its anchor and any highlight.
    ///
    /// Clipping part of the problem makes the highlight unknown while retaining
    /// the complete source and original ranges. Unknown anchors and ranges stay
    /// unknown. Empty EOF anchors are valid.
    #[must_use]
    pub fn clip(self, window: ByteRange) -> Option<Self> {
        let bytes = self.bytes.get(window.start..window.end)?.to_vec();
        let start_byte = match self.start_byte {
            Some(anchor) => Some(anchor.checked_add(window.start)?),
            None => None,
        };
        let problem = self.problem.and_then(|range| {
            if range.start < window.start || range.end > window.end {
                return None;
            }
            Some(ByteRange {
                start: range.start.checked_sub(window.start)?,
                end: range.end.checked_sub(window.start)?,
            })
        });
        Self::from_saved_parts(
            bytes,
            self.view,
            start_byte,
            problem,
            self.source_problem,
            self.original_problem,
        )
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
    pub const fn start_byte(&self) -> Option<usize> {
        self.start_byte
    }

    /// Full offending range in the named view, independent of clipping.
    #[must_use]
    pub const fn source_problem(&self) -> Option<ByteRange> {
        self.source_problem
    }

    /// Highlight range relative to the bytes actually saved in this excerpt.
    #[must_use]
    pub const fn problem(&self) -> Option<ByteRange> {
        self.problem
    }

    /// Exact retained-file range; decoded gzip bytes do not establish one.
    #[must_use]
    pub const fn original_problem(&self) -> Option<ByteRange> {
        self.original_problem
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
        assert_eq!(clipped.start_byte(), Some(8));
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

    #[test]
    fn saved_parts_preserve_independently_nullable_evidence() {
        let unknown_anchor = SourceExcerpt::from_saved_parts(
            b"abcdef".to_vec(),
            ExcerptView::RetainedOriginalBytes,
            None,
            Some(range(2, 4)),
            None,
            None,
        )
        .expect("a local highlight does not require a known view anchor");
        assert_eq!(unknown_anchor.bytes(), b"abcdef");
        assert_eq!(unknown_anchor.start_byte(), None);
        assert_eq!(unknown_anchor.problem(), Some(range(2, 4)));
        assert_eq!(unknown_anchor.source_problem(), None);
        assert_eq!(unknown_anchor.original_problem(), None);

        let source_only = SourceExcerpt::from_saved_parts(
            b"abcdef".to_vec(),
            ExcerptView::RetainedOriginalBytes,
            Some(10),
            None,
            Some(range(100, 102)),
            None,
        )
        .expect("a source span may be known without a local highlight");
        assert_eq!(source_only.start_byte(), Some(10));
        assert_eq!(source_only.problem(), None);
        assert_eq!(source_only.source_problem(), Some(range(100, 102)));
        assert_eq!(source_only.original_problem(), None);

        let original_only = SourceExcerpt::from_saved_parts(
            b"abcdef".to_vec(),
            ExcerptView::RetainedOriginalBytes,
            Some(10),
            Some(range(2, 4)),
            None,
            Some(range(12, 14)),
        )
        .expect("source and original spans are independently nullable");
        assert_eq!(original_only.problem(), Some(range(2, 4)));
        assert_eq!(original_only.source_problem(), None);
        assert_eq!(original_only.original_problem(), Some(range(12, 14)));
    }

    #[test]
    fn saved_parts_validate_only_known_coherence_and_keep_empty_eof() {
        let eof = SourceExcerpt::from_saved_parts(
            Vec::new(),
            ExcerptView::RetainedOriginalBytes,
            Some(12),
            Some(range(0, 0)),
            Some(range(12, 12)),
            Some(range(12, 12)),
        )
        .expect("an empty saved BLOB can carry an EOF anchor");
        assert!(eof.bytes().is_empty());
        assert_eq!(eof.problem(), Some(range(0, 0)));
        assert_eq!(eof.source_problem(), Some(range(12, 12)));
        assert_eq!(eof.original_problem(), Some(range(12, 12)));

        if let Ok(view_anchor) = usize::try_from(i64::MAX) {
            let at_sql_max = SourceExcerpt::from_saved_parts(
                b"x".to_vec(),
                ExcerptView::RetainedOriginalBytes,
                Some(view_anchor),
                None,
                None,
                None,
            )
            .expect("the largest SQLite INTEGER is a valid known anchor");
            assert_eq!(at_sql_max.start_byte(), Some(view_anchor));
        }

        assert!(
            SourceExcerpt::from_saved_parts(
                b"abc".to_vec(),
                ExcerptView::RetainedOriginalBytes,
                Some(10),
                Some(range(1, 2)),
                Some(range(12, 13)),
                None,
            )
            .is_none()
        );
        assert!(
            SourceExcerpt::from_saved_parts(
                b"abc".to_vec(),
                ExcerptView::RetainedOriginalBytes,
                Some(10),
                Some(range(1, 2)),
                Some(range(11, 12)),
                Some(range(12, 13)),
            )
            .is_none()
        );
        assert!(
            SourceExcerpt::from_saved_parts(
                b"abc".to_vec(),
                ExcerptView::RetainedOriginalBytes,
                None,
                Some(range(2, 4)),
                None,
                None,
            )
            .is_none()
        );
        assert!(
            SourceExcerpt::from_saved_parts(
                b"abc".to_vec(),
                ExcerptView::TransportDecodedXmlBytes,
                None,
                None,
                None,
                Some(range(1, 2)),
            )
            .is_none()
        );
        assert!(
            SourceExcerpt::from_saved_parts(
                b"abc".to_vec(),
                ExcerptView::RetainedOriginalBytes,
                Some(usize::MAX),
                Some(range(0, 1)),
                Some(range(0, 1)),
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn clipping_preserves_unknowns_and_checks_anchor_overflow() {
        let excerpt = SourceExcerpt::from_saved_parts(
            b"012345".to_vec(),
            ExcerptView::RetainedOriginalBytes,
            None,
            Some(range(2, 4)),
            None,
            None,
        )
        .expect("unknown anchor with a local highlight");
        let clipped = excerpt.clip(range(1, 5)).expect("valid clip");
        assert_eq!(clipped.bytes(), b"1234");
        assert_eq!(clipped.start_byte(), None);
        assert_eq!(clipped.problem(), Some(range(1, 3)));
        assert_eq!(clipped.source_problem(), None);
        assert_eq!(clipped.original_problem(), None);

        let partial = SourceExcerpt::from_saved_parts(
            b"012345".to_vec(),
            ExcerptView::RetainedOriginalBytes,
            Some(10),
            Some(range(2, 4)),
            Some(range(12, 14)),
            Some(range(12, 14)),
        )
        .expect("coherent known evidence");
        let clipped = partial.clip(range(3, 6)).expect("valid partial clip");
        assert_eq!(clipped.start_byte(), Some(13));
        assert_eq!(clipped.problem(), None);
        assert_eq!(clipped.source_problem(), Some(range(12, 14)));
        assert_eq!(clipped.original_problem(), Some(range(12, 14)));

        let overflow = SourceExcerpt::from_saved_parts(
            b"x".to_vec(),
            ExcerptView::RetainedOriginalBytes,
            Some(usize::MAX),
            None,
            None,
            None,
        )
        .expect("anchor itself is representable in usize");
        assert!(overflow.clip(range(1, 1)).is_none());
    }
}
