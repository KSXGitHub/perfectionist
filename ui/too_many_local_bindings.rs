// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn work(value: u32) -> u32 {
    value
}

// Bad: sixteen distinct names, one above the default limit.
fn sixteen_names(seed: u32) -> u32 {
    let one = work(seed);
    let two = work(one);
    let three = work(two);
    let four = work(three);
    let five = work(four);
    let six = work(five);
    let seven = work(six);
    let eight = work(seven);
    let nine = work(eight);
    let ten = work(nine);
    let eleven = work(ten);
    let twelve = work(eleven);
    let thirteen = work(twelve);
    let fourteen = work(thirteen);
    let fifteen = work(fourteen);
    let sixteen = work(fifteen);
    sixteen
}

// Good: exactly fifteen is not above the limit.
fn fifteen_names(seed: u32) -> u32 {
    let one = work(seed);
    let two = work(one);
    let three = work(two);
    let four = work(three);
    let five = work(four);
    let six = work(five);
    let seven = work(six);
    let eight = work(seven);
    let nine = work(eight);
    let ten = work(nine);
    let eleven = work(ten);
    let twelve = work(eleven);
    let thirteen = work(twelve);
    let fourteen = work(thirteen);
    let fifteen = work(fourteen);
    fifteen
}

// Good: a name rebound by shadowing counts once, so this binds two.
fn shadowing(seed: u32) -> u32 {
    let value = work(seed);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let value = work(value);
    let other = work(value);
    other
}

// Bad: closure parameters and `for` / `if let` / match-arm bindings
// count the same as a `let` — sixteen in all.
fn every_binding_shape(items: &[(u32, u32)], maybe: Option<u32>) -> u32 {
    let mut total = 0;
    for (left, right) in items {
        total += left + right;
    }
    if let Some(found) = maybe {
        total += found;
    }
    match maybe {
        Some(inner) => total += inner,
        None => {}
    }
    let Some(required) = maybe else {
        return total;
    };
    let mapped: Vec<u32> = items.iter().map(|(first, second)| first + second).collect();
    let (head, tail) = (mapped.first(), mapped.last());
    let sum: u32 = mapped.iter().sum();
    let count = mapped.len();
    let mean = sum / count as u32;
    let scaled = mean * required;
    let capped = scaled.min(100);
    total + capped + head.unwrap_or(&0) + tail.unwrap_or(&0)
}

// Good: names beginning with `_` are not counted, so this binds one.
fn underscored(seed: u32) -> u32 {
    let _one = work(seed);
    let _two = work(seed);
    let _three = work(seed);
    let _four = work(seed);
    let _five = work(seed);
    let _six = work(seed);
    let _seven = work(seed);
    let _eight = work(seed);
    let _nine = work(seed);
    let _ten = work(seed);
    let _eleven = work(seed);
    let _twelve = work(seed);
    let _thirteen = work(seed);
    let _fourteen = work(seed);
    let _fifteen = work(seed);
    let result = work(seed);
    result
}

// Good: a macro expansion's own bindings are not the author's.
fn formatting(seed: u32) -> String {
    let text = format!("{seed} {seed} {seed}");
    let again = format!("{text} {text}");
    again
}

fn main() {}
