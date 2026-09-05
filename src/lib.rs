//! Feature flags parsed from a small text config, evaluated per-flag against
//! a caller-supplied key (usually a user id or account id).
//!
//! Config format, one directive per line:
//!
//! ```text
//! # comment
//! flag checkout_v2 enabled=true rollout=25
//! flag dark_mode
//! override checkout_v2 user-42=true
//! ```
//!
//! `flag <name>` on its own means enabled=true, rollout=100 (a plain
//! boolean flag). `rollout` is a percentage 0..=100. `override` pins a
//! specific key to a fixed result and always wins, even over a disabled
//! flag or a 0% rollout - that's the point of an override: it's an escape
//! hatch for support and QA, not part of the normal rollout math.

use std::collections::HashMap;
use std::fmt;

/// FNV-1a, chosen over std's DefaultHasher because DefaultHasher's seed is
/// randomized per process: the same (flag, key) pair would land in a
/// different bucket every time the program restarts, which defeats the
/// point of a stable rollout.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

fn fnv1a(data: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Deterministic bucket (0..100) for a (flag, key) pair. Exposed publicly
/// so callers can show "you're in bucket N" in debug UIs without having to
/// reimplement the hash.
///
/// The NUL separator between flag and key matters: without it, flag "ab"
/// with key "c" would hash identically to flag "a" with key "bc".
pub fn bucket(flag: &str, key: &str) -> u8 {
    let mut buf = Vec::with_capacity(flag.len() + key.len() + 1);
    buf.extend_from_slice(flag.as_bytes());
    buf.push(0);
    buf.extend_from_slice(key.as_bytes());
    (fnv1a(&buf) % 100) as u8
}

#[derive(Debug, Clone)]
struct Flag {
    enabled: bool,
    rollout: u8,
    overrides: HashMap<String, bool>,
}

/// A parsed, ready-to-query set of flags.
pub struct FlagSet {
    flags: HashMap<String, Flag>,
}

impl FlagSet {
    /// Evaluate a flag for a given key. Errors if the flag name was never
    /// declared, so a typo in a call site fails loudly instead of silently
    /// resolving to "off".
    pub fn is_enabled(&self, flag: &str, key: &str) -> Result<bool, EvalError> {
        let f = self
            .flags
            .get(flag)
            .ok_or_else(|| EvalError::UnknownFlag(flag.to_string()))?;

        if let Some(&forced) = f.overrides.get(key) {
            return Ok(forced);
        }
        if !f.enabled {
            return Ok(false);
        }
        if f.rollout >= 100 {
            return Ok(true);
        }
        if f.rollout == 0 {
            return Ok(false);
        }
        Ok(bucket(flag, key) < f.rollout)
    }

    /// Names of every declared flag, in no particular order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.flags.keys().map(String::as_str)
    }

    /// Human-readable summary of a flag's static config (ignores the key
    /// used for evaluation). Returns `None` if the flag isn't declared.
    pub fn describe(&self, flag: &str) -> Option<String> {
        self.flags.get(flag).map(|f| {
            format!(
                "enabled={} rollout={} overrides={}",
                f.enabled,
                f.rollout,
                f.overrides.len()
            )
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EvalError {
    UnknownFlag(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::UnknownFlag(name) => write!(f, "unknown flag {name:?}"),
        }
    }
}

impl std::error::Error for EvalError {}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Malformed(usize),
    DuplicateFlag(usize, String),
    InvalidBool(usize, String),
    InvalidRollout(usize, String),
    UnknownAttribute(usize, String),
    UnknownDirective(usize, String),
    UnknownFlagInOverride(usize, String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Malformed(line) => write!(f, "line {line}: malformed directive"),
            ParseError::DuplicateFlag(line, name) => {
                write!(f, "line {line}: flag {name:?} already declared")
            }
            ParseError::InvalidBool(line, val) => {
                write!(f, "line {line}: expected true/false, got {val:?}")
            }
            ParseError::InvalidRollout(line, val) => {
                write!(f, "line {line}: rollout must be 0..=100, got {val:?}")
            }
            ParseError::UnknownAttribute(line, name) => {
                write!(f, "line {line}: unknown attribute {name:?}")
            }
            ParseError::UnknownDirective(line, name) => {
                write!(f, "line {line}: unknown directive {name:?}")
            }
            ParseError::UnknownFlagInOverride(line, name) => {
                write!(f, "line {line}: override for undeclared flag {name:?}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

fn parse_bool(s: &str) -> Option<bool> {
    match s {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Parse a config string into a [`FlagSet`].
///
/// A flag must be declared with `flag` before any `override` line that
/// references it - overrides are applied top to bottom, not resolved in a
/// second pass, so ordering in the file is meaningful.
pub fn parse(input: &str) -> Result<FlagSet, ParseError> {
    let mut flags: HashMap<String, Flag> = HashMap::new();

    for (idx, raw_line) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let directive = parts.next().ok_or(ParseError::Malformed(line_no))?;

        match directive {
            "flag" => {
                let name = parts
                    .next()
                    .ok_or(ParseError::Malformed(line_no))?
                    .to_string();
                if flags.contains_key(&name) {
                    return Err(ParseError::DuplicateFlag(line_no, name));
                }

                let mut enabled = true;
                let mut rollout: u8 = 100;
                for token in parts {
                    let (key, value) = token
                        .split_once('=')
                        .ok_or(ParseError::Malformed(line_no))?;
                    match key {
                        "enabled" => {
                            enabled = parse_bool(value)
                                .ok_or_else(|| ParseError::InvalidBool(line_no, value.to_string()))?;
                        }
                        "rollout" => {
                            let n: u32 = value
                                .parse()
                                .map_err(|_| ParseError::InvalidRollout(line_no, value.to_string()))?;
                            if n > 100 {
                                return Err(ParseError::InvalidRollout(line_no, value.to_string()));
                            }
                            rollout = n as u8;
                        }
                        other => {
                            return Err(ParseError::UnknownAttribute(line_no, other.to_string()))
                        }
                    }
                }

                flags.insert(
                    name,
                    Flag {
                        enabled,
                        rollout,
                        overrides: HashMap::new(),
                    },
                );
            }
            "override" => {
                let name = parts
                    .next()
                    .ok_or(ParseError::Malformed(line_no))?
                    .to_string();
                let assignment = parts.next().ok_or(ParseError::Malformed(line_no))?;
                let (key, value) = assignment
                    .split_once('=')
                    .ok_or(ParseError::Malformed(line_no))?;
                let forced = parse_bool(value)
                    .ok_or_else(|| ParseError::InvalidBool(line_no, value.to_string()))?;

                let flag = flags
                    .get_mut(&name)
                    .ok_or_else(|| ParseError::UnknownFlagInOverride(line_no, name.clone()))?;
                flag.overrides.insert(key.to_string(), forced);
            }
            other => return Err(ParseError::UnknownDirective(line_no, other.to_string())),
        }
    }

    Ok(FlagSet { flags })
}
