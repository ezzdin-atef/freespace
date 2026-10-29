pub fn fmt_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    // Roll over at 1000 so we print "0.98 MB" rather than "1003.6 KB".
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

// Only the egui UI reads this; goes away with it in phase 7.
#[cfg_attr(not(feature = "egui"), allow(dead_code))]
pub fn fmt_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Null-terminated UTF-16 string for Win32 calls.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn ends_with_ci(s: &str, suffix: &str) -> bool {
    s.len() >= suffix.len() && s.as_bytes()[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
}

/// Case-insensitive match supporting only `*`.
pub fn wild_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && p[pi] != '*' && p[pi] == n[ni] {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(fmt_size(512), "512 B");
        assert_eq!(fmt_size(1536), "1.5 KB");
        assert_eq!(fmt_count(1234567), "1,234,567");
    }

    #[test]
    fn wildcards() {
        assert!(wild_match("*", "Default"));
        assert!(wild_match("Profile *", "profile 2"));
        assert!(wild_match("MSTeams_*", "MSTeams_8wekyb3d8bbwe"));
        assert!(!wild_match("Profile *", "Default"));
    }
}
