// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn work(value: u32) -> u32 {
    value
}

// Bad: fifty-one lines of code, one above the default limit.
fn fifty_one_lines() -> u32 {
    let step_1 = work(1);
    let step_2 = work(2);
    let step_3 = work(3);
    let step_4 = work(4);
    let step_5 = work(5);
    let step_6 = work(6);
    let step_7 = work(7);
    let step_8 = work(8);
    let step_9 = work(9);
    let step_10 = work(10);
    let step_11 = work(11);
    let step_12 = work(12);
    let step_13 = work(13);
    let step_14 = work(14);
    let step_15 = work(15);
    let step_16 = work(16);
    let step_17 = work(17);
    let step_18 = work(18);
    let step_19 = work(19);
    let step_20 = work(20);
    let step_21 = work(21);
    let step_22 = work(22);
    let step_23 = work(23);
    let step_24 = work(24);
    let step_25 = work(25);
    let step_26 = work(26);
    let step_27 = work(27);
    let step_28 = work(28);
    let step_29 = work(29);
    let step_30 = work(30);
    let step_31 = work(31);
    let step_32 = work(32);
    let step_33 = work(33);
    let step_34 = work(34);
    let step_35 = work(35);
    let step_36 = work(36);
    let step_37 = work(37);
    let step_38 = work(38);
    let step_39 = work(39);
    let step_40 = work(40);
    let step_41 = work(41);
    let step_42 = work(42);
    let step_43 = work(43);
    let step_44 = work(44);
    let step_45 = work(45);
    let step_46 = work(46);
    let step_47 = work(47);
    let step_48 = work(48);
    let step_49 = work(49);
    let step_50 = work(50);
    step_51
}

// Good: fifty lines of code is not above the limit.
fn fifty_lines() -> u32 {
    let step_1 = work(1);
    let step_2 = work(2);
    let step_3 = work(3);
    let step_4 = work(4);
    let step_5 = work(5);
    let step_6 = work(6);
    let step_7 = work(7);
    let step_8 = work(8);
    let step_9 = work(9);
    let step_10 = work(10);
    let step_11 = work(11);
    let step_12 = work(12);
    let step_13 = work(13);
    let step_14 = work(14);
    let step_15 = work(15);
    let step_16 = work(16);
    let step_17 = work(17);
    let step_18 = work(18);
    let step_19 = work(19);
    let step_20 = work(20);
    let step_21 = work(21);
    let step_22 = work(22);
    let step_23 = work(23);
    let step_24 = work(24);
    let step_25 = work(25);
    let step_26 = work(26);
    let step_27 = work(27);
    let step_28 = work(28);
    let step_29 = work(29);
    let step_30 = work(30);
    let step_31 = work(31);
    let step_32 = work(32);
    let step_33 = work(33);
    let step_34 = work(34);
    let step_35 = work(35);
    let step_36 = work(36);
    let step_37 = work(37);
    let step_38 = work(38);
    let step_39 = work(39);
    let step_40 = work(40);
    let step_41 = work(41);
    let step_42 = work(42);
    let step_43 = work(43);
    let step_44 = work(44);
    let step_45 = work(45);
    let step_46 = work(46);
    let step_47 = work(47);
    let step_48 = work(48);
    let step_49 = work(49);
    step_50
}

// Good: blank lines and comment-only lines are not code, so this body
// has forty-nine lines of code across seventy source lines.
fn commented_and_spaced() -> u32 {
    // step 1
    let step_1 = work(1);

    // step 2
    let step_2 = work(2);

    // step 3
    let step_3 = work(3);

    // step 4
    let step_4 = work(4);

    // step 5
    let step_5 = work(5);

    // step 6
    let step_6 = work(6);

    // step 7
    let step_7 = work(7);

    // step 8
    let step_8 = work(8);

    // step 9
    let step_9 = work(9);

    // step 10
    let step_10 = work(10);

    // step 11
    let step_11 = work(11);

    // step 12
    let step_12 = work(12);

    // step 13
    let step_13 = work(13);

    // step 14
    let step_14 = work(14);

    // step 15
    let step_15 = work(15);

    // step 16
    let step_16 = work(16);

    // step 17
    let step_17 = work(17);

    // step 18
    let step_18 = work(18);

    // step 19
    let step_19 = work(19);

    // step 20
    let step_20 = work(20);

    // step 21
    let step_21 = work(21);

    // step 22
    let step_22 = work(22);

    // step 23
    let step_23 = work(23);

    // step 24
    let step_24 = work(24);

    // step 25
    let step_25 = work(25);

    // step 26
    let step_26 = work(26);

    // step 27
    let step_27 = work(27);

    // step 28
    let step_28 = work(28);

    // step 29
    let step_29 = work(29);

    // step 30
    let step_30 = work(30);

    // step 31
    let step_31 = work(31);

    // step 32
    let step_32 = work(32);

    // step 33
    let step_33 = work(33);

    // step 34
    let step_34 = work(34);

    // step 35
    let step_35 = work(35);

    // step 36
    let step_36 = work(36);

    // step 37
    let step_37 = work(37);

    // step 38
    let step_38 = work(38);

    // step 39
    let step_39 = work(39);

    // step 40
    let step_40 = work(40);

    // step 41
    let step_41 = work(41);

    // step 42
    let step_42 = work(42);

    // step 43
    let step_43 = work(43);

    // step 44
    let step_44 = work(44);

    // step 45
    let step_45 = work(45);

    // step 46
    let step_46 = work(46);

    // step 47
    let step_47 = work(47);

    // step 48
    let step_48 = work(48);
    // The steps above are followed by a summary.

    /* And this block comment,
       spanning three lines,
       is not code either. */
    step_49
}

fn main() {}
