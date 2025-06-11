use unicode_normalization::UnicodeNormalization;

/// Converts a heading into a CommonMark/GFM-compatible slug.
///
/// Example: `Intro - description` → `intro---description`
pub fn slugify(input: &str) -> String {
    let normalized = input.nfkd().collect::<String>().to_lowercase();
    let mut slug = String::new();

    for c in normalized.chars() {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if c.is_whitespace() || is_gfm_punctuation(c) {
            slug.push('-');
        }
        // skip all other punctuation/symbols
    }

    // Don't strip or collapse multiple dashes
    slug
}

/// Punctuation characters commonly mapped to '-' in GFM
fn is_gfm_punctuation(c: char) -> bool {
    matches!(
        c,
        '!' | '"'
            | '#'
            | '$'
            | '%'
            | '&'
            | '('
            | ')'
            | '*'
            | '+'
            | ','
            | '.'
            | '/'
            | ':'
            | ';'
            | '<'
            | '='
            | '>'
            | '@'
            | '['
            | '\\'
            | ']'
            | '^'
            | '_'
            | '`'
            | '{'
            | '|'
            | '}'
            | '~'
            | '-'
    )
}
