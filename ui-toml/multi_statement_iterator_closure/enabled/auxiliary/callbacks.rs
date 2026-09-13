// compile-flags: --crate-type=lib

pub fn separate_module() {
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled
    });
}
