pub fn production(seed: u32) -> u32 {
    let one = seed;
    let two = one;
    let three = two;
    let four = three;
    let five = four;
    let six = five;
    let seven = six;
    let eight = seven;
    let nine = eight;
    let ten = nine;
    let eleven = ten;
    let twelve = eleven;
    let thirteen = twelve;
    let fourteen = thirteen;
    let fifteen = fourteen;
    let sixteen = fifteen;
    sixteen
}

#[cfg(test)]
mod tests {
    pub fn cfg_test_helper(seed: u32) -> u32 {
        let one = seed;
        let two = one;
        let three = two;
        let four = three;
        let five = four;
        let six = five;
        let seven = six;
        let eight = seven;
        let nine = eight;
        let ten = nine;
        let eleven = ten;
        let twelve = eleven;
        let thirteen = twelve;
        let fourteen = thirteen;
        let fifteen = fourteen;
        let sixteen = fifteen;
        sixteen
    }

    #[test]
    fn test_function() {
        let one = 1;
        let two = one;
        let three = two;
        let four = three;
        let five = four;
        let six = five;
        let seven = six;
        let eight = seven;
        let nine = eight;
        let ten = nine;
        let eleven = ten;
        let twelve = eleven;
        let thirteen = twelve;
        let fourteen = thirteen;
        let fifteen = fourteen;
        let sixteen = fifteen;
        assert_eq!(cfg_test_helper(sixteen), 1);
    }
}
