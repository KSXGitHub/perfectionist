#![allow(dead_code, unused, reason = "fixture")]

pub fn wanted(line: &str) -> bool {
    !line.is_empty()
}

pub fn longer(line: &str, limit: usize) -> bool {
    line.len() > limit
}

pub fn shorter(line: &str, limit: usize) -> bool {
    line.len() < limit * 2
}

pub fn three_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| wanted(line)).filter(|line| line.starts_with('#')).filter(|line| line.ends_with('!')).collect()
}

pub fn holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| wanted(line)).filter(|line| (line.starts_with('#') || line.ends_with('!'))).collect()
}

pub fn moves_a_capture(limit: usize, mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.filter(move |line| longer(line, limit)).any(move |line| shorter(line, limit))
}

pub fn takes_a_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.take_while(|line| wanted(line)).take_while(|line| line.starts_with('#')).collect()
}

pub fn an_option(line: Option<&'static str>) -> bool {
    line.filter(|line| wanted(line)).is_some_and(|line| line.starts_with('#'))
}

pub fn holds_a_comment(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) /* both */ && line.starts_with('#'))
        .collect()
}
