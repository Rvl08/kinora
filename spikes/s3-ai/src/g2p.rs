//! Text -> phonemes for Kokoro without espeak-ng (GPL-3.0).
//!
//! English: misaki's American dictionaries (Apache-2.0; the phoneme set Kokoro
//! was trained on), with a small rule fallback for out-of-vocabulary words.
//! Indonesian: orthography rules (the spelling is close to phonemic) and
//! penultimate stress.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::text::expand_numbers;

#[derive(Default, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct G2pStats {
    pub words: usize,
    pub oov: usize,
    pub oov_words: Vec<String>,
}

pub struct English {
    dict: HashMap<String, String>,
}

impl English {
    pub fn load(gold: &Path, silver: &Path) -> Result<Self, String> {
        let mut dict = HashMap::new();
        // Silver first, gold second: gold wins.
        for path in [silver, gold] {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            for (k, val) in v.as_object().ok_or("dictionary is not an object")? {
                let ph = match val {
                    Value::String(s) => Some(s.clone()),
                    Value::Object(m) => m
                        .get("DEFAULT")
                        .or_else(|| m.values().find(|x| x.is_string()))
                        .and_then(|x| x.as_str())
                        .map(str::to_owned),
                    _ => None,
                };
                if let Some(ph) = ph {
                    dict.insert(k.clone(), ph);
                }
            }
        }
        Ok(Self { dict })
    }

    fn lookup(&self, word: &str) -> Option<String> {
        let lower = word.to_lowercase();
        let mut cap = lower.clone();
        if let Some(f) = cap.get_mut(0..1) {
            f.make_ascii_uppercase();
        }
        self.dict
            .get(word)
            .or_else(|| self.dict.get(&lower))
            .or_else(|| self.dict.get(&cap))
            .cloned()
            .or_else(|| {
                // Plural / possessive of a known word.
                let base = lower
                    .strip_suffix("'s")
                    .or_else(|| lower.strip_suffix('s'))?;
                self.dict.get(base).map(|p| format!("{p}z"))
            })
    }

    pub fn phonemize(&self, text: &str, stats: &mut G2pStats) -> String {
        let text = expand_numbers(text, "en");
        let mut out = String::new();
        for token in tokenize(&text) {
            match token {
                Token::Word(w) => {
                    stats.words += 1;
                    let ph = self.lookup(&w).unwrap_or_else(|| {
                        stats.oov += 1;
                        stats.oov_words.push(w.clone());
                        english_rules(&w)
                    });
                    if !out.is_empty() && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(&ph);
                }
                Token::Punct(c) => out.push(c),
            }
        }
        out
    }
}

enum Token {
    Word(String),
    Punct(char),
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() || c == '\'' || c == '-' {
            word.push(c);
        } else {
            if !word.is_empty() {
                out.push(Token::Word(std::mem::take(&mut word)));
            }
            if ",.!?;:".contains(c) {
                out.push(Token::Punct(c));
            }
        }
    }
    if !word.is_empty() {
        out.push(Token::Word(word));
    }
    out
}

/// Last-resort English letter-to-sound rules (out-of-vocabulary words only).
fn english_rules(word: &str) -> String {
    let w = word.to_lowercase();
    if !w.is_ascii() {
        return w; // the byte-wise rules below are ASCII-only
    }
    let b = w.as_bytes();
    let mut out = String::new();
    let mut stressed = false;
    let mut i = 0;
    while i < b.len() {
        let two = if i + 1 < b.len() { &w[i..i + 2] } else { "" };
        let (ph, n) = match two {
            "ch" => ("ʧ", 2),
            "sh" => ("ʃ", 2),
            "th" => ("θ", 2),
            "ng" => ("ŋ", 2),
            "ph" => ("f", 2),
            "ee" | "ea" => ("i", 2),
            "oo" => ("u", 2),
            "ou" => ("W", 2),
            _ => (
                match b[i] {
                    b'a' => "æ",
                    b'e' => "ɛ",
                    b'i' => "ɪ",
                    b'o' => "ɑ",
                    b'u' => "ʌ",
                    b'y' => "i",
                    b'c' => "k",
                    b'j' => "ʤ",
                    b'q' => "k",
                    b'x' => "ks",
                    b'r' => "ɹ",
                    b'g' => "ɡ",
                    _ => &w[i..i + 1],
                },
                1,
            ),
        };
        if !stressed && "æɛɪɑʌiuW".contains(ph) {
            out.push('ˈ');
            stressed = true;
        }
        out.push_str(ph);
        i += n;
    }
    out
}

/// Indonesian words spelt with e but pronounced /e/ or /ɛ/ (taling) rather
/// than the default schwa (pepet). Small on purpose: a spike, not a lexicon.
const ID_TALING: &[&str] = &[
    "meja", "enak", "ekor", "besok", "sore", "merah", "teh", "nenek", "desa", "pesta", "bebek",
    "sate", "kereta", "mereka", "dewa", "eropa", "elang", "ember", "becak", "lele", "tempe",
    "pena", "boleh", "oleh", "kakek",
];

fn is_vowel(c: char) -> bool {
    "aiueoəɛ".contains(c)
}

/// Indonesian spelling -> phonemes, with penultimate stress.
pub fn indonesian(text: &str, stats: &mut G2pStats) -> String {
    let text = expand_numbers(text, "id");
    let mut out = String::new();
    for token in tokenize(&text) {
        match token {
            Token::Word(w) => {
                stats.words += 1;
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(&indonesian_word(&w.to_lowercase()));
            }
            Token::Punct(c) => out.push(c),
        }
    }
    out
}

fn indonesian_word(w: &str) -> String {
    let taling = ID_TALING.contains(&w);
    let chars: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
    let mut ph: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let last = i + 1 == chars.len();
        let (p, n): (String, usize) = match (c, next) {
            ('n', Some('g')) => ("ŋ".into(), 2),
            ('n', Some('y')) => ("ɲ".into(), 2),
            ('s', Some('y')) => ("ʃ".into(), 2),
            ('k', Some('h')) => ("x".into(), 2),
            ('a', Some('i')) if i + 2 == chars.len() => ("ai".into(), 2),
            ('a', Some('u')) if i + 2 == chars.len() => ("au".into(), 2),
            ('c', _) => ("ʧ".into(), 1),
            ('j', _) => ("ʤ".into(), 1),
            ('y', _) => ("j".into(), 1),
            ('g', _) => ("ɡ".into(), 1),
            ('q', _) => ("k".into(), 1),
            ('v', _) => ("f".into(), 1),
            ('x', _) => ("ks".into(), 1),
            ('k', _) if last => ("ʔ".into(), 1),
            ('e', _) => (if taling { "e".into() } else { "ə".into() }, 1),
            ('é', _) => ("e".into(), 1),
            _ => (c.to_string(), 1),
        };
        ph.push(p);
        i += n;
    }
    // Stress the penultimate vowel (the last one when the penultimate is a schwa).
    let vowels: Vec<usize> = ph
        .iter()
        .enumerate()
        .filter(|(_, p)| p.chars().next().is_some_and(is_vowel))
        .map(|(i, _)| i)
        .collect();
    if vowels.len() >= 2 {
        let mut v = vowels[vowels.len() - 2];
        if ph[v] == "ə" {
            v = vowels[vowels.len() - 1];
        }
        // Mark before the syllable onset (the consonant right before the vowel).
        let at = if v > 0 && !ph[v - 1].chars().next().is_some_and(is_vowel) {
            v - 1
        } else {
            v
        };
        ph.insert(at, "ˈ".into());
    }
    ph.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indonesian_rules() {
        let mut s = G2pStats::default();
        assert_eq!(indonesian("nyanyi", &mut s), "ˈɲaɲi");
        assert_eq!(indonesian_word("tidak"), "ˈtidaʔ");
        assert_eq!(indonesian_word("bangun"), "ˈbaŋun");
        assert_eq!(indonesian_word("syarat"), "ˈʃarat");
        assert_eq!(indonesian_word("meja"), "ˈmeʤa");
        assert_eq!(indonesian_word("kelapa"), "kəˈlapa");
    }
}
