//! Just enough JSON for the small, known replies here: find a key, read the value after it.
//! Keys are passed with their quotes and colon (and a `[` for lists), e.g. `"\"sunrise\":["`.

use alloc::string::String;

/// What follows the first `key` in `s`.
pub fn after<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    s.find(key).map(|i| &s[i + key.len()..])
}

/// The number right after `key`, as text (`None` for `null` or anything else).
pub fn number<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let rest = after(s, key)?;
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// The `n`th entry of the list after `key` (which ends in `[`), as text (`None` for `null`, or
/// past the end).
pub fn nth_number<'a>(s: &'a str, key: &str, n: usize) -> Option<&'a str> {
    let list = after(s, key)?;
    let list = &list[..list.find(']')?];
    let item = list.split(',').nth(n)?.trim();
    let is_number = !item.is_empty()
        && item
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'.');
    is_number.then_some(item)
}

/// The string after `key`, e.g. `"\"media_title\":"` (`None` if it's `null` or missing).
pub fn string_value(s: &str, key: &str) -> Option<String> {
    let rest = after(s, key)?.strip_prefix('"')?;
    Some(string(rest)?.0)
}

/// A JSON string from just after its opening quote: the text, and what follows the closing quote.
pub fn string(s: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &s[i + 1..])),
            '\\' => match chars.next()?.1 {
                'u' => {
                    let mut code = 0;
                    for _ in 0..4 {
                        code = code * 16 + chars.next()?.1.to_digit(16)?;
                    }
                    // Halves of an escaped emoji aren't chars; they'd be dropped anyway.
                    out.extend(char::from_u32(code));
                }
                'n' | 'r' | 't' | 'b' | 'f' => out.push(' '),
                escaped => out.push(escaped),
            },
            c => out.push(c),
        }
    }
    None
}
