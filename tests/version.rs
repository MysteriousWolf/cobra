//! The release version scheme, checked against the crate's own version so a bad bump
//! fails `cargo test` rather than the release workflow.
//!
//! cobra is versioned `YY.N.P`: the last two digits of the release year, the count of
//! releases made in that year starting at 1, and a patch number. `26.1.0` is the first
//! release of 2026, `26.1.1` a patch on it and `26.2.0` the second release of the year.
//!
//! Because the three fields still form a valid semver triple, Cargo's usual
//! compatibility rules apply: `26.1.1` is a drop-in for `26.1.0`, and `26.2.0` is a
//! compatible upgrade from `26.1.x`. A new year bumps the major, which is exactly the
//! breaking-change signal Cargo expects a yearly line to be allowed to make.
//!
//! `ci/version.fish` enforces the rest of the policy — that a release is strictly newer
//! than the last one — since that needs the repository's tags.

/// A parsed `YY.N.P` version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    year: u32,
    release: u32,
    patch: u32,
}

impl Version {
    /// Parses the strict three-field form. No pre-release or build metadata: a release
    /// that ships is either in the year line or it is not.
    fn parse(s: &str) -> Result<Self, String> {
        let fields: Vec<&str> = s.split('.').collect();
        if fields.len() != 3 {
            return Err(format!("{s:?} is not YY.N.P (expected three fields, got {})", fields.len()));
        }
        let mut parsed = [0u32; 3];
        for (slot, field) in parsed.iter_mut().zip(&fields) {
            if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!("{s:?} has a non-numeric field {field:?}"));
            }
            if field.len() > 1 && field.starts_with('0') {
                return Err(format!("{s:?} has a zero-padded field {field:?}"));
            }
            *slot = field.parse().map_err(|e| format!("{s:?}: {e}"))?;
        }
        Ok(Self { year: parsed[0], release: parsed[1], patch: parsed[2] })
    }

    /// The calendar year this version claims to be a release of.
    fn calendar_year(self) -> u32 {
        2000 + self.year
    }
}

/// The current UTC year, from the system clock (civil-from-days, Howard Hinnant).
fn current_year() -> u32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the system clock is before 1970")
        .as_secs();
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let y = yoe + era * 400;
    (if mp >= 10 { y + 1 } else { y }) as u32
}

#[test]
fn the_crate_version_follows_the_year_scheme() {
    let raw = env!("CARGO_PKG_VERSION");
    let v = Version::parse(raw).unwrap_or_else(|e| panic!("Cargo.toml version: {e}"));
    assert!(v.release >= 1, "{raw}: the first release of a year is .1, not .0");
    assert!(v.calendar_year() >= 2026, "{raw}: cobra's first year-versioned release is 2026");
    assert!(
        v.calendar_year() <= current_year(),
        "{raw} claims to be a {} release, but it is only {}",
        v.calendar_year(),
        current_year(),
    );
}

#[test]
fn versions_order_the_way_cargo_would() {
    let v = |s: &str| Version::parse(s).unwrap();
    assert!(v("26.1.0") < v("26.1.1"));
    assert!(v("26.1.9") < v("26.2.0"));
    assert!(v("26.9.9") < v("27.1.0"));
    assert!(v("26.10.0") > v("26.9.0"), "release counts compare numerically, not as text");
    assert_eq!(v("26.1.0"), v("26.1.0"));
}

#[test]
fn malformed_versions_are_rejected() {
    for bad in ["26.1", "26.1.0.0", "26.1.0-rc.1", "26.1.x", "26..0", "v26.1.0", "26.01.0", ""] {
        assert!(Version::parse(bad).is_err(), "{bad:?} should not parse");
    }
    assert_eq!(Version::parse("26.1.0").unwrap(), Version { year: 26, release: 1, patch: 0 });
    assert_eq!(Version::parse("26.1.0").unwrap().calendar_year(), 2026);
}

#[test]
fn the_clock_gives_a_plausible_year() {
    let y = current_year();
    assert!((2024..2200).contains(&y), "current_year() returned {y}");
}
