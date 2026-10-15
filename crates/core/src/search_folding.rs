use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

pub(crate) fn fold(input: &str) -> String {
    let mut folded = String::with_capacity(input.len());
    for character in input
        .nfd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
    {
        match character {
            'đ' | 'ð' => folded.push('d'),
            'ł' => folded.push('l'),
            'ø' => folded.push('o'),
            'æ' => folded.push_str("ae"),
            'œ' => folded.push_str("oe"),
            'ß' => folded.push_str("ss"),
            'þ' => folded.push_str("th"),
            other => folded.push(other),
        }
    }
    folded
}
