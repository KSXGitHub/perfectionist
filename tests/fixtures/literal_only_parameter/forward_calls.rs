pub fn exercise(flag: bool, input: Option<bool>) {
    computed_bool(flag);
    computed_bool(true);
    computed_option(input);
    computed_option(None);
    literal_bool(true);
    let function = escaped;
    function(flag);
    escaped(true);
    State.computed_method(flag);
    State.computed_method(true);
}

fn computed_bool(flag: bool) { let _ = flag; }
fn computed_option(input: Option<bool>) { let _ = input; }
fn literal_bool(flag: bool) { let _ = flag; }
fn escaped(flag: bool) { let _ = flag; }
struct State;
impl State {
    fn computed_method(&self, flag: bool) { let _ = flag; }
}

pub fn literal_call_sites() {
    computed_bool(true);
    computed_option(None);
    escaped(false);
    State.computed_method(false);
}
