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

pub fn record(length: usize) {}

pub fn label(length: usize) -> &'static str {
    match length {
        0 => "empty",
        _ => "filled",
    }
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

pub fn chained_into_a_consumer(lines: std::vec::IntoIter<&'static str>) {
    lines.map(str::trim).map(str::len).for_each(record);
}

pub fn chained_into_a_predicate(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.map(str::trim).any(str::is_empty)
}

pub fn chained_with_a_turbofish(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim).map(str::parse::<usize>).filter_map(Result::ok)
        .collect()
}

pub fn chained_from_an_owned_item(headers: std::vec::IntoIter<String>) -> Vec<&'static str> {
    headers.map(|header| header.len()).map(label).collect()
}

pub fn chained_into_a_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + line.trim().len())
}
