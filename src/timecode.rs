//! Parsing and formatting of `HH:MM:SS(.mmm)` time codes as used by FFmpeg.

/// Formats seconds for display: `M:SS`, or `H:MM:SS` past an hour, with tenths when `precise`.
pub fn format_short(seconds: f64, precise: bool) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    let s = tenths / 10;
    let base = if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    };
    if precise {
        format!("{base}.{}", tenths % 10)
    } else {
        base
    }
}

/// Formats seconds with milliseconds: `HH:MM:SS.mmm`.
pub fn format_precise(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let s = total_ms / 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        s / 3600,
        (s / 60) % 60,
        s % 60,
        total_ms % 1000
    )
}

/// Seconds as an FFmpeg argument (`-ss`, `-t`): plain seconds with millisecond precision.
pub fn arg(seconds: f64) -> String {
    format!("{:.3}", seconds.max(0.0))
}

/// Snaps `seconds` to the start of the closest frame of a `fps` video lasting `duration` seconds (at most
/// the start of its last frame).
pub fn snap(seconds: f64, fps: f64, duration: f64) -> f64 {
    let fps = fps.max(1.0);
    let max = (duration - 1.0 / fps).max(0.0);
    ((seconds * fps).round() / fps).clamp(0.0, max)
}

/// Parses `HH:MM:SS(.fff)`, `MM:SS(.fff)` or plain seconds.
pub fn parse(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for (index, part) in parts.iter().enumerate() {
        let value: f64 = if index == parts.len() - 1 {
            part.parse().ok()?
        } else {
            f64::from(part.parse::<u32>().ok()?)
        };
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        total = total * 60.0 + value;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_for_display() {
        assert_eq!(format_short(0.0, false), "0:00");
        assert_eq!(format_short(83.46, true), "1:23.5");
        assert_eq!(format_short(3725.0, false), "1:02:05");
        assert_eq!(format_precise(61.5), "00:01:01.500");
        assert_eq!(arg(12.3456), "12.346");
    }

    #[test]
    fn parses_time_codes() {
        assert_eq!(parse("00:53:30"), Some(3210.0));
        assert_eq!(parse("01:02:05.250"), Some(3725.25));
        assert_eq!(parse("02:13"), Some(133.0));
        assert_eq!(parse("12.5"), Some(12.5));
        assert_eq!(parse(""), None);
        assert_eq!(parse("1:2:3:4"), None);
        assert_eq!(parse("-3"), None);
    }

    #[test]
    fn snaps_to_frames() {
        assert_eq!(snap(1.01, 30.0, 10.0), 1.0);
        assert!((snap(100.0, 30.0, 10.0) - (10.0 - 1.0 / 30.0)).abs() < 1e-9);
        assert_eq!(snap(-1.0, 30.0, 10.0), 0.0);
    }
}
