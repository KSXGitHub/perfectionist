fn benchmark_binder(seed: u32) -> u32 {
    let one = seed + 1;
    let two = one + 1;
    let three = two + 1;
    let four = three + 1;
    let five = four + 1;
    let six = five + 1;
    let seven = six + 1;
    let eight = seven + 1;
    let nine = eight + 1;
    let ten = nine + 1;
    let eleven = ten + 1;
    let twelve = eleven + 1;
    let thirteen = twelve + 1;
    let fourteen = thirteen + 1;
    let fifteen = fourteen + 1;
    let sixteen = fifteen + 1;
    sixteen
}

#[test]
fn works() {
    assert_eq!(benchmark_binder(0), 16);
}
