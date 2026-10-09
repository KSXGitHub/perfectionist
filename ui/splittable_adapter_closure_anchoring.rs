// edition:2024
//
// Where the chain sits in the body, which is the gate whose wrong answer
// compiles rather than failing to. A chain the closure does not always
// reach would run for every item once lifted, and anything observable
// before it would be observed in a different order.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(text: &str) -> usize {
    text.len()
}

// Not flagged: a chain the closure does not always reach. Lifting it
// would run the parse for every item rather than for none, which
// compiles and answers differently.
fn conditional(flag: bool, lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| if flag { parse(line.trim()) } else { 0 })
        .collect()
}

// Not flagged: the right operand of `&&` is skipped where the left
// settles the answer.
fn short_circuit(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.fold(false, |seen, line| seen && line.trim().is_empty())
}

// Bad: a `let` and the block holding it run when the block does, so
// the chain's position is the block's.
fn fold_with_a_let(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let length = line.trim().len();
        total + length
    })
}

// Good: the steps lifted, which leaves the `let` nothing to name and the
// closure the accumulation alone.
fn split_fold_with_a_let(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .fold(0, |total, length| total + length)
}

// Bad: a `match` evaluates its scrutinee before any arm, so a chain
// there is reached whenever the closure is.
fn scrutinee_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| match line.trim().len() {
        0 => total,
        length => total + length,
    })
}

// Good: the steps lifted, leaving the `match` its own item to scrutinise.
fn split_scrutinee_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .fold(0, |total, length| match length {
            0 => total,
            length => total + length,
        })
}

// Bad: an `if` evaluates its condition before either branch, so a chain
// there is reached whenever the closure is.
fn condition_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        if line.trim().is_empty() {
            total
        } else {
            total + 1
        }
    })
}

// Good: the steps lifted, leaving the `if` the answer they computed.
fn split_condition_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::is_empty)
        .fold(0, |total, empty| if empty { total } else { total + 1 })
}

// Not flagged: a branch of an `if` does not run every time the closure
// does.
fn branching_fold(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        if flag {
            total + line.trim().len()
        } else {
            total
        }
    })
}

// Not flagged: a `let`'s `else` block runs only where the pattern does
// not match.
fn let_else_fold(fallback: Option<usize>, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let Some(extra) = fallback else {
            return total + line.trim().len();
        };
        total + extra
    })
}

// Not flagged: an arm of a `match` runs only where its pattern is the one
// that matched.
fn arm_fold(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| match flag {
        true => total + line.trim().len(),
        false => total,
    })
}

// Not flagged: a `loop` runs its body any number of times, including
// none, so a chain inside one is not a chain the closure always reaches.
fn looping_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| loop {
        break total + line.trim().len();
    })
}

// Bad: `?` is a `match` on its operand, and an operand runs.
fn fold_through_try(
    mut lines: std::vec::IntoIter<&'static str>,
) -> Result<usize, std::num::ParseIntError> {
    lines.try_fold(0usize, |total, line| Ok(total + line.trim().parse::<usize>()?))
}

// Good: the steps lifted, leaving the closure the `?` the reader wrote.
fn split_fold_through_try(
    lines: std::vec::IntoIter<&'static str>,
) -> Result<usize, std::num::ParseIntError> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .try_fold(0usize, |total, parsed| Ok(total + parsed?))
}

// Not flagged: an early `return` leaves the closure before the chain,
// so a leading `map` would run the step for every item where the
// closure ran it for none.
fn early_return_fold(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        if flag {
            return total;
        }
        total + line.trim().len()
    })
}

// Not flagged: a `?` before the chain can leave the closure first, so a
// leading `map` would run the step for every item where the closure ran it
// for none.
fn try_before_the_chain(
    first: Option<usize>,
    mut lines: std::vec::IntoIter<&'static str>,
) -> Option<usize> {
    lines.try_fold(0, |total, line| Some(total + first? + line.trim().len()))
}

// Not flagged: one step, where the `?` the reader wrote is not a second
// one. It lowers to a call the chain is the argument of, which no `map`
// could hold.
fn one_step_through_try(
    mut lines: std::vec::IntoIter<&'static str>,
) -> Result<usize, std::num::ParseIntError> {
    lines.try_fold(0usize, |total, line| Ok(total + line.parse::<usize>()?))
}

// Not flagged: a `loop` runs its body any number of times, including
// none, which is what declines a chain inside one. The `break` here sits
// after the chain, so what declines it is the `loop` and not a diversion.
fn loop_after_the_chain(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let mut sum = total;
        loop {
            sum += line.trim().len();
            break;
        }
        sum
    })
}

// Not flagged: a `let`'s `else` block, reached where nothing diverts
// before the chain, so the arm reading `else` is what declines it.
fn let_else_diverting_after(
    fallback: Option<usize>,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines.fold(0, |total, line| {
        let Some(extra) = fallback else {
            let length = line.trim().len();
            return total + length;
        };
        total + extra
    })
}

// Not flagged: a `panic!` before the chain leaves the closure without
// evaluating it, which a leading `map` would do for every item. What leaves
// is read from the type rather than from the name it is written as.
fn panics_before_the_chain(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        if flag {
            panic!("stop");
        }
        total + line.trim().len()
    })
}

// Not flagged: an arm that diverges leaves the closure before the chain,
// for whichever scrutinee reaches it. A `panic!` expands in `core`, so what
// runs first cannot be read from a span.
fn panics_in_an_arm(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        match flag {
            true => panic!("stop"),
            false => {}
        }
        total + line.trim().len()
    })
}

// Not flagged: an assignment evaluates its right side before the place it
// writes to, so a divergence there runs before a chain in the index.
fn diverges_through_an_assignment(
    mut slots: Vec<usize>,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines.fold(0, |total, line| {
        slots[line.trim().len()] = std::process::exit(7);
        total
    })
}

// Not flagged: a `let` before the chain runs its initialiser, so a
// divergence there leaves the closure first.
fn diverges_in_a_let(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let extra = if flag { std::process::exit(3) } else { 1 };
        total + extra + line.trim().len()
    })
}

// Not flagged: an `exit` never returns, which the rule reads from its type
// rather than from the name it is called by.
fn exits_before_the_chain(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        if flag {
            std::process::exit(0);
        }
        total + line.trim().len()
    })
}

// Not flagged: a unary closure whose body is not the chain. The scope
// here is a closure whose whole job is the chain.
fn body_is_not_the_chain(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|line| line.trim().len() + 1).collect()
}

struct Pair {
    first: usize,
    second: usize,
}

fn pair(count: usize) -> Pair {
    Pair {
        first: count,
        second: count,
    }
}

fn counted(total: usize) -> usize {
    total
}

// Bad: a struct expression's field is a position the chain is always
// reached through, and every field before it runs first.
fn folds_into_a_struct_field(lines: std::vec::IntoIter<&'static str>) -> Pair {
    lines.fold(
        Pair {
            first: 0,
            second: 0,
        },
        |acc, line| Pair {
            first: acc.first + line.trim().len(),
            second: acc.second,
        },
    )
}

// Good: the steps lifted, leaving the field the accumulation.
fn split_folds_into_a_struct_field(lines: std::vec::IntoIter<&'static str>) -> Pair {
    lines.map(str::trim).map(str::len).fold(
        Pair {
            first: 0,
            second: 0,
        },
        |acc, length| Pair {
            first: acc.first + length,
            second: acc.second,
        },
    )
}

// Not flagged: a `let`-`else` before the chain leaves the closure exactly
// where its pattern does not match.
fn diverges_in_a_let_else(
    fallback: Option<usize>,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines.fold(0, |total, line| {
        let Some(extra) = fallback else {
            return total;
        };
        total + extra + line.trim().len()
    })
}

// Not flagged: a struct expression evaluates its fields before its
// `..base`, so a divergence in a field runs before a chain in the base.
fn diverges_in_a_struct_field(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let built = Pair {
            first: if flag { std::process::exit(1) } else { 0 },
            ..pair(line.trim().len())
        };
        total + built.second
    })
}

// Not flagged: the accumulator's side of a stateful closure runs before
// the chain and after it once the chain is lifted, so a call there keeps
// the answer and moves the trace.
fn accumulator_has_an_effect(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| counted(total) + line.trim().len())
}

// Not flagged: a division by zero panics without being a call, so the
// item's steps would run before the panic that stopped them.
fn divides_before_the_chain(divisor: usize, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + 1 / divisor + line.trim().len())
}

// Not flagged: a compound division panics without being a call too, and
// is a shape of its own: the arm reading the plain spelling does not
// reach it.
fn divides_in_place_before_the_chain(
    divisor: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines.fold(0, |total, line| {
        let mut scaled = total;
        scaled /= divisor;
        scaled + line.trim().len()
    })
}

// Not flagged: an index out of bounds panics without being a call, so
// the item's steps would run before the panic that stopped them.
fn indexes_before_the_chain(table: [usize; 2], lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + table[total] + line.trim().len())
}

fn main() {}
