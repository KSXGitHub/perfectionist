#[cfg(all(test, debug_assertions))]
fn conjunction(conjunction_param: &str) -> String {
    conjunction_param.to_owned()
}

#[cfg(not(not(test)))]
fn double_negation(double_negation_param: &str) -> String {
    double_negation_param.to_owned()
}

#[cfg(any(test, target_pointer_width = "64"))]
fn disjunction(disjunction_param: &str) -> String {
    disjunction_param.to_owned()
}

// `docsrs` is a cfg rustc knows about and nothing sets, so it stands in
// for "a condition that is false in every build here" without drawing
// an `unexpected_cfgs` warning.
//
// `not(all(not(test), docsrs))` holds in a non-test build too — the
// conjunction is false either way — so it is production code.
#[cfg(not(all(not(test), docsrs)))]
fn negated_conjunction(negated_conjunction_param: &str) -> String {
    negated_conjunction_param.to_owned()
}

// `not(any(not(test), docsrs))` is false as soon as `test` is off, so
// it is test-only. Under De Morgan the negated `any` behaves like an
// `all`, which is the arm this exercises.
#[cfg(not(any(not(test), docsrs)))]
fn negated_disjunction(negated_disjunction_param: &str) -> String {
    negated_disjunction_param.to_owned()
}
