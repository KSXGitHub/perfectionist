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
