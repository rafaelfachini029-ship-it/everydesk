//! Memorable machine codes (`palavra-palavra-NN`, e.g. `vela-rio-37`) used as the engine id.
//!
//! The host compares the login username with `Config::get_id()` byte for byte, so codes are
//! always lowercase ASCII; what a person dictates on the phone goes through [`normalize`].

use hbb_common::rand::{self, Rng};

const WORDS_TXT: &str = include_str!("words-pt.txt");

fn words() -> Vec<&'static str> {
    WORDS_TXT
        .lines()
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect()
}

/// Longest code the engine accepts as a custom id (`^[a-zA-Z][\w-]{5,15}$`).
const MAX_LEN: usize = 16;
/// Combined letters of the two words.
const MAX_WORD_LETTERS: usize = 10;

/// `palavra-palavra-NN`: two dictionary words (≤ 10 letters together) and 10..=99.
pub fn generate_code(rng: &mut impl Rng) -> String {
    let ws = words();
    loop {
        let a = ws[rng.gen_range(0..ws.len())];
        let b = ws[rng.gen_range(0..ws.len())];
        if a == b || a.len() + b.len() > MAX_WORD_LETTERS {
            continue;
        }
        let n: u8 = rng.gen_range(10..=99);
        return format!("{a}-{b}-{n}");
    }
}

/// Engine hook signature (`fn() -> String`): a fresh code from the thread rng.
pub fn new_code() -> String {
    generate_code(&mut rand::thread_rng())
}

/// Shape check only (`aaa-bbb-NN`, lowercase), so a code survives dictionary edits.
pub fn is_valid_code(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let [a, b, n] = parts[..] else {
        return false;
    };
    let word = |w: &str| (3..=5).contains(&w.len()) && w.bytes().all(|c| c.is_ascii_lowercase());
    s.len() <= MAX_LEN
        && word(a)
        && word(b)
        && n.len() == 2
        && n.bytes().all(|c| c.is_ascii_digit())
        && !n.starts_with('0')
}

/// What a person types after hearing a code: trim, lowercase, accents dropped, and any run of
/// spaces, underscores or dashes becomes a single `-` (`"Água Lua 42"` → `"agua-lua-42"`).
pub fn normalize(s: &str) -> String {
    strip_accents(s)
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-')
        .filter(|p| !p.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

/// NFD then drop combining marks, for the letters a Portuguese keyboard (or phone dictation)
/// produces: precomposed Latin letters with diacritics map to their base letter, and
/// combining marks (U+0300..=U+036F, already-decomposed input) are removed. Without a
/// Unicode-tables dependency; other characters pass through (and fail `is_valid_code`).
fn strip_accents(s: &str) -> String {
    s.chars()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .map(base_letter)
        .collect()
}

fn base_letter(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'ç' => 'c',
        'Ç' => 'C',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn generated_code_matches_engine_regex() {
        let re = hbb_common::regex::Regex::new(r"^[a-z][a-z0-9-]{5,15}$").unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1000 {
            let code = generate_code(&mut rng);
            assert!(re.is_match(&code), "{code}");
            assert!(hbb_common::is_valid_custom_id(&code), "{code}");
            assert!(code.len() <= MAX_LEN, "{code}");
            let parts: Vec<&str> = code.split('-').collect();
            assert_eq!(parts.len(), 3, "{code}");
            assert!(
                parts[0].len() + parts[1].len() <= MAX_WORD_LETTERS,
                "{code}"
            );
            let nn: u8 = parts[2].parse().unwrap();
            assert!((10..=99).contains(&nn), "{code}");
            assert!(is_valid_code(&code), "{code}");
        }
    }

    #[test]
    fn normalize_accepts_phone_dictation() {
        assert_eq!(normalize("Azul Tigre 42"), "azul-tigre-42");
        assert_eq!(normalize("  vela_rio   37 "), "vela-rio-37");
        assert_eq!(normalize("VELA-RIO-37"), "vela-rio-37");
        assert!(is_valid_code(&normalize("Azul Tigre 42")));
    }

    #[test]
    fn normalize_strips_accents() {
        assert_eq!(normalize("Água Lua 42"), "agua-lua-42");
        assert_eq!(normalize("CORAÇÃO pé 37"), "coracao-pe-37");
        // Already decomposed (NFD) input: base letter + combining acute.
        assert_eq!(normalize("A\u{301}gua lua 42"), "agua-lua-42");
        assert_eq!(normalize("ÍMÃ õ ü ñ"), "ima-o-u-n");
    }

    #[test]
    fn is_valid_code_rejects_other_ids() {
        assert!(!is_valid_code("259402742"));
        assert!(!is_valid_code(""));
        assert!(!is_valid_code("Vela-rio-37"));
        assert!(!is_valid_code("vela-rio-7"));
        assert!(!is_valid_code("vela-rio-100"));
        assert!(!is_valid_code("vela-37"));
        assert!(!is_valid_code("ve-rio-37"));
        assert!(!is_valid_code("vela-rio-37-1"));
    }

    #[test]
    fn dictionary_has_no_duplicates_and_lengths_3_to_5() {
        let ws = words();
        assert!(ws.len() >= 300, "{}", ws.len());
        let mut seen = std::collections::HashSet::new();
        for w in &ws {
            assert!((3..=5).contains(&w.len()), "{w}");
            assert!(w.bytes().all(|b| b.is_ascii_lowercase()), "{w}");
            assert!(seen.insert(*w), "duplicate {w}");
        }
    }
}
