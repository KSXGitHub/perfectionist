fn helper(build_script_param: &str) -> String {
    build_script_param.to_owned()
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-env=GREETING={}", helper("a"));
}
