fn helper(example_param: &str) -> String {
    example_param.to_owned()
}

fn main() {
    println!("{}", helper("a"));
}
