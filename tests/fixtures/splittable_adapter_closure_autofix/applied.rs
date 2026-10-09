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
    lines.filter(|line| wanted(line) && line.starts_with('#') && line.ends_with('!')).collect()
}

pub fn holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| wanted(line) && (line.starts_with('#') || line.ends_with('!'))).collect()
}

pub fn moves_a_capture(limit: usize, mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(move |line| longer(line, limit) && shorter(line, limit))
}

pub fn takes_a_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.take_while(|line| wanted(line) && line.starts_with('#')).collect()
}

pub fn an_option(line: Option<&'static str>) -> bool {
    line.is_some_and(|line| wanted(line) && line.starts_with('#'))
}

pub fn holds_a_comment(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) /* both */ && line.starts_with('#'))
        .collect()
}

pub fn chained_into_a_consumer(lines: std::vec::IntoIter<&'static str>) {
    lines.for_each(|line| record(line.trim().len()));
}

pub fn chained_into_a_predicate(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| line.trim().is_empty())
}

pub fn chained_with_a_turbofish(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.trim().parse::<usize>().ok())
        .collect()
}

pub fn chained_from_an_owned_item(headers: std::vec::IntoIter<String>) -> Vec<&'static str> {
    headers.map(|header| label(header.len())).collect()
}

pub fn chained_into_a_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + line.trim().len())
}

pub mod held {
    pub struct Wrapper(pub usize);

    impl Wrapper {
        pub fn value(self) -> usize {
            self.0
        }

        pub fn doubled(self) -> Wrapper {
            Wrapper(self.0 * 2)
        }
    }
}

pub fn two_unimported_steps(items: Vec<held::Wrapper>) -> Vec<usize> {
    items
        .into_iter()
        .map(|wrapper| wrapper.doubled().value())
        .collect()
}
