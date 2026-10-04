const NUL_MARKER: &str = "__RECOVERED_NUL__";

/// A single No-Intro export with one recovered NUL for each native owner kind.
///
/// Keeping the document as Rust text avoids committing XML containing literal
/// NUL bytes while preserving their exact source positions after generation.
pub fn native_owner_nul_document() -> String {
    concat!(
        "<!-- document before datafile: é😀__RECOVERED_NUL__ -->\r\n",
        "<datafile>\r\n",
        "  <header><!-- header comment __RECOVERED_NUL__ -->",
        "<author>Author __RECOVERED_NUL__</author></header>\r\n",
        "  <game name=\"Game __RECOVERED_NUL__\">\r\n",
        "    <archive name=\"Archive __RECOVERED_NUL__\"/>\r\n",
        "    <source><!-- source comment __RECOVERED_NUL__ -->",
        "<details comment1=\"Source details __RECOVERED_NUL__\"/>",
        "<serials media_serial1=\"Source serials __RECOVERED_NUL__\"/>",
        "<file forcename=\"Source file __RECOVERED_NUL__\"/>",
        "</source>\r\n",
        "    <release><!-- release comment __RECOVERED_NUL__ -->",
        "<details comment=\"Release details __RECOVERED_NUL__\"/>",
        "<serials box_serial=\"Release serials __RECOVERED_NUL__\"/>",
        "<file forcename=\"Release file __RECOVERED_NUL__\"/>",
        "</release>\r\n",
        "  </game>\r\n",
        "</datafile>\r\n",
        "<!-- document after datafile: __RECOVERED_NUL__ -->\r\n",
    )
    .replace(NUL_MARKER, "\0")
}
