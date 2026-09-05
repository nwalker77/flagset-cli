use flagset_cli::{bucket, parse, EvalError, ParseError};

const CONFIG: &str = "
# a mix of the shapes real config files end up taking
flag always_on rollout=100
flag always_off enabled=false
flag half rollout=50
flag off_but_overridden enabled=false
override off_but_overridden vip=true
flag on_but_overridden
override on_but_overridden banned=false
";

#[test]
fn eval_cases() {
    let flags = parse(CONFIG).expect("config should parse");

    struct Case {
        desc: &'static str,
        flag: &'static str,
        key: &'static str,
        want: Result<bool, EvalError>,
    }

    let cases = vec![
        Case {
            desc: "100% rollout is always on",
            flag: "always_on",
            key: "anyone",
            want: Ok(true),
        },
        Case {
            desc: "100% rollout with an empty key is still on",
            flag: "always_on",
            key: "",
            want: Ok(true),
        },
        Case {
            desc: "disabled flag is off regardless of key",
            flag: "always_off",
            key: "anyone",
            want: Ok(false),
        },
        Case {
            desc: "override wins over enabled=false",
            flag: "off_but_overridden",
            key: "vip",
            want: Ok(true),
        },
        Case {
            desc: "keys without an override fall back to the base flag",
            flag: "off_but_overridden",
            key: "randomer",
            want: Ok(false),
        },
        Case {
            desc: "override wins over a 100% rollout",
            flag: "on_but_overridden",
            key: "banned",
            want: Ok(false),
        },
        Case {
            desc: "unrelated key ignores the override",
            flag: "on_but_overridden",
            key: "randomer",
            want: Ok(true),
        },
        Case {
            desc: "unknown flag is an error, not a silent false",
            flag: "does_not_exist",
            key: "anyone",
            want: Err(EvalError::UnknownFlag("does_not_exist".to_string())),
        },
    ];

    for c in cases {
        let got = flags.is_enabled(c.flag, c.key);
        assert_eq!(got, c.want, "case failed: {}", c.desc);
    }
}

#[test]
fn empty_key_is_a_valid_key_not_a_special_case() {
    let flags = parse(CONFIG).expect("config should parse");
    // an empty key must hash and bucket the same as any other string, and
    // a 100% rollout must still cover it.
    assert_eq!(flags.is_enabled("always_on", ""), Ok(true));
}

#[test]
fn rollout_is_deterministic_across_separate_parses() {
    let a = parse(CONFIG).expect("config should parse");
    let b = parse(CONFIG).expect("config should parse");
    for key in ["u1", "u2", "u3", "", "a very long key with spaces"] {
        assert_eq!(
            a.is_enabled("half", key),
            b.is_enabled("half", key),
            "bucket for {key:?} should be stable across parses"
        );
    }
}

#[test]
fn rollout_matches_the_public_bucket_function() {
    let flags = parse(CONFIG).expect("config should parse");
    for key in ["u1", "u2", "u3", "u4", "u5"] {
        let want = bucket("half", key) < 50;
        assert_eq!(flags.is_enabled("half", key), Ok(want));
    }
}

#[test]
fn flag_names_are_case_sensitive() {
    let flags = parse("flag Beta\n").expect("config should parse");
    assert_eq!(
        flags.is_enabled("beta", "k"),
        Err(EvalError::UnknownFlag("beta".to_string()))
    );
    assert_eq!(flags.is_enabled("Beta", "k"), Ok(true));
}

#[test]
fn comments_and_blank_lines_are_ignored() {
    let flags = parse("\n  \n# just a comment\nflag solo\n\n").expect("config should parse");
    assert_eq!(flags.is_enabled("solo", "k"), Ok(true));
}

#[test]
fn parse_error_cases() {
    struct Case {
        desc: &'static str,
        input: &'static str,
        want: ParseError,
    }

    let cases = vec![
        Case {
            desc: "duplicate flag declaration",
            input: "flag a\nflag a\n",
            want: ParseError::DuplicateFlag(2, "a".to_string()),
        },
        Case {
            desc: "override before the flag it targets exists",
            input: "override never_declared k=true\n",
            want: ParseError::UnknownFlagInOverride(1, "never_declared".to_string()),
        },
        Case {
            desc: "rollout above 100",
            input: "flag a rollout=150\n",
            want: ParseError::InvalidRollout(1, "150".to_string()),
        },
        Case {
            desc: "rollout that isn't a number",
            input: "flag a rollout=soon\n",
            want: ParseError::InvalidRollout(1, "soon".to_string()),
        },
        Case {
            desc: "enabled value that isn't true/false",
            input: "flag a enabled=maybe\n",
            want: ParseError::InvalidBool(1, "maybe".to_string()),
        },
        Case {
            desc: "flag directive with no name",
            input: "flag\n",
            want: ParseError::Malformed(1),
        },
        Case {
            desc: "attribute with no value",
            input: "flag a enabled\n",
            want: ParseError::Malformed(1),
        },
        Case {
            desc: "unrecognized attribute",
            input: "flag a color=blue\n",
            want: ParseError::UnknownAttribute(1, "color".to_string()),
        },
        Case {
            desc: "unrecognized top-level directive",
            input: "flga a\n",
            want: ParseError::UnknownDirective(1, "flga".to_string()),
        },
    ];

    for c in cases {
        let got = parse(c.input).expect_err("expected a parse error");
        assert_eq!(got, c.want, "case failed: {}", c.desc);
    }
}
