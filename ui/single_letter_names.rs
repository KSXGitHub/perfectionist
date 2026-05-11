#![allow(dead_code, unused_variables, unused_assignments, dropping_copy_types)]

// Bad: generic type parameter with a single letter on a long function.
fn map_pairs<K, V>(input: Vec<(K, V)>) -> Vec<(V, K)> {
    let mut output = Vec::new();
    for (key, value) in input {
        output.push((value, key));
    }
    output
}

trait Trivial {
    fn done(&self);
}

// OK: short trait impl, single-letter generic permitted.
impl<T> Trivial for Option<T> {
    fn done(&self) {}
}

// Bad: long trait impl, even though it implements a trait.
impl<T> Iterator for Wrapper<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.exhausted {
            return None;
        }
        self.exhausted = true;
        self.value.take()
    }
    fn count(self) -> usize {
        usize::from(self.value.is_some() && !self.exhausted)
    }
    fn last(self) -> Option<T> {
        if self.exhausted {
            None
        } else {
            self.value
        }
    }
    fn nth(&mut self, _index: usize) -> Option<T> {
        self.next()
    }
}

// Bad: single-letter generic on a free-standing item.
struct Wrapper<T> {
    value: Option<T>,
    exhausted: bool,
}

fn run() {
    // Bad: single-letter `let` binding outside test code.
    let m = 5_u32;
    // OK: `n` is in the default `let` allowlist.
    let n = 10_u32;

    let items = vec![3_i32, 1, 2];

    // OK: comparison closure with single-letter parameters as a single
    // expression callback.
    let mut sorted = items.clone();
    sorted.sort_by(|a, b| a.cmp(b));

    // OK: trivial wrapper closure (single method call on the parameter).
    let _doubled: Vec<i32> = sorted.iter().map(|x| x.abs()).collect();

    // OK: trivial wrapper closure (field access).
    let pairs = vec![Pair { left: 1, right: 2 }];
    let _lefts: Vec<i32> = pairs.iter().map(|p| p.left).collect();

    // Bad: multi-statement closure body, single-letter parameter.
    let _formatted: Vec<String> = sorted
        .iter()
        .map(|t| {
            let label = format!("value {t}");
            label
        })
        .collect();

    let _used = m + n;
}

struct Pair {
    left: i32,
    right: i32,
}

// Bad: single-letter function parameters outside the conventional set.
fn write_label(w: &mut String, t: &str) {
    w.push_str(t);
}

// OK: `n` is in the default function-parameter allowlist.
fn take_n(n: usize) -> usize {
    n + 1
}

// OK: `f` is in the default allowlist (formatter convention).
struct DisplayMe;
impl std::fmt::Display for DisplayMe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DisplayMe")
    }
}

#[cfg(test)]
mod tests {
    // OK: single-letter `let` bindings under `#[cfg(test)]` are exempt.
    fn fixtures() {
        let a = 1_u32;
        let b = 2_u32;
        let c = a + b;
        let _ = c;
    }
}

fn main() {
    run();
}
