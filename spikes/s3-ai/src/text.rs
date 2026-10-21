//! Text utilities shared by ASR scoring and TTS front ends: normalisation,
//! number expansion (English, Indonesian), word error rate, word alignment.

const EN_ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const EN_TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn en_below_100(n: u64) -> String {
    if n < 20 {
        EN_ONES[n as usize].to_owned()
    } else if n % 10 == 0 {
        EN_TENS[(n / 10) as usize].to_owned()
    } else {
        format!(
            "{} {}",
            EN_TENS[(n / 10) as usize],
            EN_ONES[(n % 10) as usize]
        )
    }
}

/// English cardinal; four-digit numbers 1100-2099 read as years ("twenty twenty six").
pub fn en_number(n: u64, year_style: bool) -> String {
    if year_style && (1100..2100).contains(&n) && n % 100 != 0 {
        let hi = n / 100;
        let lo = n % 100;
        let lo_s = if lo < 10 {
            format!("oh {}", EN_ONES[lo as usize])
        } else {
            en_below_100(lo)
        };
        return format!("{} {}", en_below_100(hi), lo_s);
    }
    match n {
        0..=99 => en_below_100(n),
        100..=999 => {
            let rest = n % 100;
            let head = format!("{} hundred", EN_ONES[(n / 100) as usize]);
            if rest == 0 {
                head
            } else {
                format!("{head} {}", en_below_100(rest))
            }
        }
        1000..=999_999 => {
            let rest = n % 1000;
            let head = format!("{} thousand", en_number(n / 1000, false));
            if rest == 0 {
                head
            } else {
                format!("{head} {}", en_number(rest, false))
            }
        }
        _ => n.to_string(),
    }
}

const ID_ONES: [&str; 10] = [
    "nol", "satu", "dua", "tiga", "empat", "lima", "enam", "tujuh", "delapan", "sembilan",
];

/// Indonesian cardinal (Kamus Besar style): sebelas, seratus, seribu, ...
pub fn id_number(n: u64) -> String {
    match n {
        0..=9 => ID_ONES[n as usize].to_owned(),
        10 => "sepuluh".into(),
        11 => "sebelas".into(),
        12..=19 => format!("{} belas", ID_ONES[(n - 10) as usize]),
        20..=99 => {
            let r = n % 10;
            let head = format!("{} puluh", ID_ONES[(n / 10) as usize]);
            if r == 0 {
                head
            } else {
                format!("{head} {}", ID_ONES[r as usize])
            }
        }
        100..=999 => {
            let head = if n / 100 == 1 {
                "seratus".to_owned()
            } else {
                format!("{} ratus", ID_ONES[(n / 100) as usize])
            };
            let r = n % 100;
            if r == 0 {
                head
            } else {
                format!("{head} {}", id_number(r))
            }
        }
        1000..=999_999 => {
            let head = if n / 1000 == 1 {
                "seribu".to_owned()
            } else {
                format!("{} ribu", id_number(n / 1000))
            };
            let r = n % 1000;
            if r == 0 {
                head
            } else {
                format!("{head} {}", id_number(r))
            }
        }
        1_000_000..=999_999_999 => {
            let head = format!("{} juta", id_number(n / 1_000_000));
            let r = n % 1_000_000;
            if r == 0 {
                head
            } else {
                format!("{head} {}", id_number(r))
            }
        }
        _ => n.to_string(),
    }
}

/// Replaces digit runs with words (English years when they look like years).
pub fn expand_numbers(text: &str, lang: &str) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    let flush = |digits: &mut String, out: &mut String| {
        if digits.is_empty() {
            return;
        }
        let n: u64 = digits.parse().unwrap_or(0);
        let w = if lang == "id" {
            id_number(n)
        } else {
            en_number(n, digits.len() == 4)
        };
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push_str(&w);
        out.push(' ');
        digits.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if (c == ',' || c == '.') && !digits.is_empty() {
            // thousands separators inside numbers ("1,000" / "1.000")
            continue;
        } else {
            flush(&mut digits, &mut out);
            out.push(c);
        }
    }
    flush(&mut digits, &mut out);
    out
}

/// Lower case, numbers as words, punctuation removed, single spaces.
pub fn normalize(text: &str, lang: &str) -> Vec<String> {
    let t = expand_numbers(text, lang).to_lowercase();
    let t: String = t
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect();
    t.split_whitespace()
        .map(|w| w.trim_matches('\'').to_owned())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word-level Levenshtein alignment: (reference index, hypothesis index) for
/// every matched or substituted pair, plus the edit count.
pub fn align(reference: &[String], hypothesis: &[String]) -> (Vec<(usize, usize, bool)>, usize) {
    let (n, m) = (reference.len(), hypothesis.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = d[i - 1][j - 1] + usize::from(reference[i - 1] != hypothesis[j - 1]);
            d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let same = reference[i - 1] == hypothesis[j - 1];
        if d[i][j] == d[i - 1][j - 1] + usize::from(!same) {
            pairs.push((i - 1, j - 1, same));
            i -= 1;
            j -= 1;
        } else if d[i][j] == d[i - 1][j] + 1 {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();
    (pairs, d[n][m])
}

/// Word error rate (edits / reference words).
pub fn wer(reference: &[String], hypothesis: &[String]) -> f64 {
    if reference.is_empty() {
        return if hypothesis.is_empty() { 0.0 } else { 1.0 };
    }
    align(reference, hypothesis).1 as f64 / reference.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(en_number(2026, true), "twenty twenty six");
        assert_eq!(en_number(400, false), "four hundred");
        assert_eq!(id_number(116), "seratus enam belas");
        assert_eq!(id_number(2026), "dua ribu dua puluh enam");
        assert_eq!(
            normalize("In 2026, 400 hours!", "en").join(" "),
            "in twenty twenty six four hundred hours"
        );
    }

    #[test]
    fn word_error_rate() {
        let r = normalize("the quick brown fox", "en");
        let h = normalize("the quick brown box jumps", "en");
        assert!((wer(&r, &h) - 0.5).abs() < 1e-9);
    }
}
