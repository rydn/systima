//! Compact, width-stable humanisers that write into a caller-supplied
//! `String`, so hot render paths can reuse one scratch buffer.

use std::fmt::Write as _;

const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];

/// Humanise a byte count: `512B`, `1.2K`, `34M`, `9.8G` (≤ 5 chars).
pub fn bytes(out: &mut String, b: u64) {
    out.clear();
    push_bytes(out, b);
}

fn push_bytes(out: &mut String, b: u64) {
    if b < 1024 {
        let _ = write!(out, "{b}B");
        return;
    }
    let mut v = b as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if v < 10.0 {
        let _ = write!(out, "{v:.1}{}", UNITS[unit]);
    } else {
        let _ = write!(out, "{v:.0}{}", UNITS[unit]);
    }
}

/// Humanise a bytes-per-second rate: `1.2M/s`.
pub fn rate(out: &mut String, bps: u64) {
    out.clear();
    push_bytes(out, bps);
    out.push_str("/s");
}

/// Tenths of a percent → `42.3%`; values ≥ 100% drop the decimal.
pub fn pct_tenths(out: &mut String, tenths: u64) {
    out.clear();
    if tenths >= 1000 {
        let _ = write!(out, "{}%", tenths / 10);
    } else {
        let _ = write!(out, "{}.{}%", tenths / 10, tenths % 10);
    }
}

/// Tenths of a percent → whole percent `42%` (axis labels).
pub fn pct_whole(out: &mut String, tenths: u64) {
    out.clear();
    let _ = write!(out, "{}%", tenths / 10);
}

/// Seconds → `3d 04:12:33` or `04:12:33`.
pub fn duration(out: &mut String, secs: u64) {
    out.clear();
    let days = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if days > 0 {
        let _ = write!(out, "{days}d {h:02}:{m:02}:{s:02}");
    } else {
        let _ = write!(out, "{h:02}:{m:02}:{s:02}");
    }
}

/// Seconds → short process time `12:34` (m:s) or `3h07` or `2d`.
pub fn short_duration(out: &mut String, secs: u64) {
    out.clear();
    if secs < 3600 {
        let _ = write!(out, "{}:{:02}", secs / 60, secs % 60);
    } else if secs < 86_400 {
        let _ = write!(out, "{}h{:02}", secs / 3600, (secs % 3600) / 60);
    } else {
        let _ = write!(out, "{}d{:02}h", secs / 86_400, (secs % 86_400) / 3600);
    }
}

/// Round `max` up to a "nice" ceiling on a 1-2-5 ladder within binary
/// units, so axis labels read as `2K`, `50M`, `1G` rather than `1.37M`.
pub fn nice_ceiling(max: u64) -> u64 {
    if max <= 1 {
        return 1;
    }
    let mut unit: u64 = 1;
    while max / unit >= 1024 && unit <= 1 << 50 {
        unit *= 1024;
    }
    let mant = max.div_ceil(unit);
    const LADDER: [u64; 10] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1024];
    for step in LADDER {
        if mant <= step {
            return step.saturating_mul(unit);
        }
    }
    1024u64.saturating_mul(unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(f: impl Fn(&mut String)) -> String {
        let mut out = String::new();
        f(&mut out);
        out
    }

    #[test]
    fn bytes_humanised() {
        assert_eq!(s(|o| bytes(o, 0)), "0B");
        assert_eq!(s(|o| bytes(o, 1023)), "1023B");
        assert_eq!(s(|o| bytes(o, 1024)), "1.0K");
        assert_eq!(s(|o| bytes(o, 1536)), "1.5K");
        assert_eq!(s(|o| bytes(o, 10 * 1024)), "10K");
        assert_eq!(s(|o| bytes(o, 1 << 30)), "1.0G");
        assert_eq!(s(|o| bytes(o, 987 * (1 << 20))), "987M");
        assert_eq!(s(|o| rate(o, 2 * (1 << 20))), "2.0M/s");
    }

    #[test]
    fn percent_and_durations() {
        assert_eq!(s(|o| pct_tenths(o, 0)), "0.0%");
        assert_eq!(s(|o| pct_tenths(o, 423)), "42.3%");
        assert_eq!(s(|o| pct_tenths(o, 1000)), "100%");
        assert_eq!(s(|o| pct_tenths(o, 2345)), "234%");
        assert_eq!(
            s(|o| duration(o, 3 * 86_400 + 4 * 3600 + 12 * 60 + 33)),
            "3d 04:12:33"
        );
        assert_eq!(s(|o| duration(o, 59)), "00:00:59");
        assert_eq!(s(|o| short_duration(o, 754)), "12:34");
        assert_eq!(s(|o| short_duration(o, 3 * 3600 + 7 * 60)), "3h07");
        assert_eq!(s(|o| short_duration(o, 2 * 86_400 + 3600)), "2d01h");
    }

    #[test]
    fn nice_ceiling_ladder() {
        assert_eq!(nice_ceiling(0), 1);
        assert_eq!(nice_ceiling(1), 1);
        assert_eq!(nice_ceiling(3), 5);
        assert_eq!(nice_ceiling(7), 10);
        assert_eq!(nice_ceiling(1023), 1024);
        assert_eq!(nice_ceiling(1024), 1024);
        assert_eq!(nice_ceiling(1025), 2 * 1024);
        assert_eq!(nice_ceiling(1_400_000), 2 * 1024 * 1024);
        assert_eq!(nice_ceiling(600 * 1024 * 1024), 1024 * 1024 * 1024);
        assert_eq!(nice_ceiling(1001 * 1024), 1024 * 1024);
        assert!(nice_ceiling(u64::MAX) > u64::MAX / 2);
    }
}
