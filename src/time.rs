/* -----------------------------------------------------------------------------
 * Timestamp Generation
 *
 * Provides human-readable timestamps for snapshot filenames. Uses the
 * 'date' command when available for locale-aware formatting, falls back to
 * a manual Julian Day Number conversion when not.
 *
 * The Julian Day conversion algorithm (L25-43) is the inverse of the standard
 * conversion from civil date to JDN, used here to recover year/month/day from
 * a Unix timestamp when the 'date' command is unavailable.
 * -------------------------------------------------------------------------- */

use std::time::{SystemTime, UNIX_EPOCH};

/* --- primary API --- */

pub fn local_now() -> (i64, u32, u32, u32, u32) {
    let out = std::process::Command::new("date")
        .arg("+%Y %m %d %H %M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok());

    if let Some(s) = out {
        let p: Vec<&str> = s.trim().split_whitespace().collect();
        if p.len() == 5 {
            if let (Ok(y), Ok(mo), Ok(d), Ok(h), Ok(mi)) = (
                p[0].parse::<i64>(),
                p[1].parse::<u32>(),
                p[2].parse::<u32>(),
                p[3].parse::<u32>(),
                p[4].parse::<u32>(),
            ) {
                return (y, mo, d, h, mi);
            }
        }
    }

    // THEORY: Convert Unix epoch to civil date via Julian Day Number
    //   1. Add offset to convert Unix days (1970-01-01) to Julian days (4713 BC)
    //   2. Apply Gregorian calendar correction (the +32044 and divisions by 146097)
    //   3. Recover year/month/day from the JDN using the inverse formula
    // This is the algorithm from "Calendrical Calculations" by Reingold & Dershowitz.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let tod = secs % 86400;
    let hh = (tod / 3600) as u32;
    let mm = ((tod % 3600) / 60) as u32;
    let j = days as i64 + 2440588;
    let a = j + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - (146097 * b) / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;
    let day = (e - (153 * m + 2) / 5 + 1) as u32;
    let month = (m + 3 - 12 * (m / 10)) as u32;
    let year = 100 * b + d - 4800 + m / 10;
    (year, month, day, hh, mm)
}

/* --- formatting --- */

pub fn current_timestamp() -> String {
    let (y, mo, d, h, mi) = local_now();
    format!("{:04}-{:02}-{:02}_{:02}-{:02}", y, mo, d, h, mi)
}
