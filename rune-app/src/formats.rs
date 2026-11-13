//! Text the collection pages show as React's helpers format it: dates added (TrackList
//! formatDate), long release dates (AlbumView formatReleaseDateLong), total durations
//! (formatTotalDuration), compact counts (Intl compact notation), quality badges
//! (getMediaQualityBadge) and an artist page's album subtitles (getArtistAlbumSubtitle).

use serde_json::Value;

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Days since 1970-01-01 for a civil date (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// (year, month 1-12, day) of a "YYYY-MM-DD…" string.
fn ymd(s: &str) -> Option<(i64, i64, i64)> {
    let y = s.get(0..4)?.parse().ok()?;
    let m: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// Seconds since the epoch of an ISO 8601 timestamp as TIDAL writes them
/// ("2024-01-01T00:00:00.000+0000", "…Z", "…+02:00"), or of a bare date (midnight UTC).
pub fn timestamp(s: &str) -> Option<i64> {
    let (y, m, d) = ymd(s)?;
    let mut secs = days_from_civil(y, m, d) * 86400;
    let Some(rest) = s.get(10..).and_then(|r| r.strip_prefix('T').or(r.strip_prefix(' '))) else { return Some(secs) };
    let hms: Vec<i64> = rest.get(0..8).unwrap_or("").split(':').filter_map(|x| x.parse().ok()).collect();
    if let [h, mi, se] = hms[..] {
        secs += h * 3600 + mi * 60 + se;
    }
    // the zone: after the seconds and any fraction
    let zone = rest.find(['+', '-', 'Z']).map(|i| &rest[i..]).unwrap_or("");
    if let Some(sign) = zone.chars().next().filter(|c| *c == '+' || *c == '-') {
        let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
        if digits.len() >= 4 {
            let off = digits[0..2].parse::<i64>().unwrap_or(0) * 3600 + digits[2..4].parse::<i64>().unwrap_or(0) * 60;
            secs -= if sign == '+' { off } else { -off };
        }
    }
    Some(secs)
}

/// A track's date added (TrackList formatDate): "This week", "Last week", "Last month" within
/// 7, 14 and 30 days (either way, rounded up), otherwise "Jan 5, 2024". Empty without a date.
pub fn date_added(s: &str, now: i64) -> String {
    let Some(at) = timestamp(s) else { return String::new() };
    let days = ((now - at).abs() + 86399) / 86400;
    match days {
        ..=7 => "This week".into(),
        8..=14 => "Last week".into(),
        15..=30 => "Last month".into(),
        _ => {
            let (y, m, d) = civil(at.div_euclid(86400));
            format!("{} {d}, {y}", &MONTHS[m as usize - 1][..3])
        }
    }
}

/// (year, month, day) of a day count since 1970 (civil_from_days).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// An album's release date in its footer: "JANUARY 5, 2024" (the date as written, no zone).
pub fn release_long(s: &str) -> String {
    match ymd(s) {
        Some((y, m, d)) => format!("{} {d}, {y}", MONTHS[m as usize - 1].to_uppercase()),
        None => s.to_uppercase(),
    }
}

/// formatTotalDuration: "h:mm:ss", or "m:ss" under an hour.
pub fn total_duration(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// "N TRACKS (m:ss)" (AlbumView's header and footer).
pub fn track_count(n: usize, secs: u64) -> String {
    let mut s = format!("{n} TRACK{}", if n == 1 { "" } else { "S" });
    if secs > 0 {
        s = format!("{s} ({})", total_duration(secs));
    }
    s
}

/// Intl.NumberFormat("en", {notation: "compact", maximumFractionDigits: 1}): 950, 1.2K, 12.3K,
/// 1M.
pub fn compact(n: u64) -> String {
    const UNITS: [(f64, &str); 4] = [(1e3, "K"), (1e6, "M"), (1e9, "B"), (1e12, "T")];
    let x = n as f64;
    let Some(mut i) = UNITS.iter().rposition(|(u, _)| x >= *u) else { return n.to_string() };
    let mut v = (x / UNITS[i].0 * 10.0).round() / 10.0;
    if v >= 1000.0 && i + 1 < UNITS.len() {
        i += 1;
        v = (x / UNITS[i].0 * 10.0).round() / 10.0;
    }
    let num = if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") };
    format!("{num}{}", UNITS[i].1)
}

/// The fans line on an artist page: compact, and none for no fans.
pub fn fans(n: u64) -> Option<String> {
    (n > 0).then(|| format!("{} fans", compact(n)))
}

/// getMediaQualityBadge: the label and whether it's the top tier ("max") or lossless ("hifi").
pub fn quality_badge(d: &Value) -> Option<(&'static str, u8)> {
    let tags: Vec<&str> = d["mediaMetadata"]["tags"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if tags.contains(&"HIRES_LOSSLESS") {
        return Some(("HI-RES LOSSLESS", 2));
    }
    if tags.contains(&"LOSSLESS") {
        return Some(("LOSSLESS", 1));
    }
    match d["audioQuality"].as_str()? {
        "HI_RES_LOSSLESS" | "HI_RES" => Some(("HI-RES LOSSLESS", 2)),
        "LOSSLESS" => Some(("LOSSLESS", 1)),
        _ => Some(("HIGH", 0)),
    }
}

/// getArtistAlbumSubtitle: "2026 · Album" on its artist's pages, "Various Artists · 2026" when
/// the artist only appears on it; None without a release year.
pub fn artist_album_subtitle(d: &Value, artist: u64) -> Option<String> {
    let year = d["releaseDate"].as_str().and_then(|r| r.get(0..4)).filter(|y| y.chars().all(|c| c.is_ascii_digit()))?;
    let artists: Vec<&Value> = match d["artists"].as_array() {
        Some(a) => a.iter().collect(),
        None if d["artist"].is_object() => vec![&d["artist"]],
        None => vec![],
    };
    if !artists.is_empty() && !artists.iter().any(|a| a["id"].as_u64() == Some(artist)) {
        let names: Vec<&str> = artists.iter().filter_map(|a| a["name"].as_str()).collect();
        return Some(format!("{} · {year}", names.join(", ")));
    }
    Some(match d["type"].as_str() {
        Some("ALBUM") => format!("{year} · Album"),
        Some("EP") => format!("{year} · EP"),
        Some("SINGLE") => format!("{year} · Single"),
        _ => year.to_string(),
    })
}

/// A multi-disc album's tracks by volume (AlbumView volumeGroups): (volume, indices), in the
/// order volumes first appear; None for a single volume.
pub fn volumes(tracks: &[Value]) -> Option<Vec<(u64, Vec<usize>)>> {
    let mut out: Vec<(u64, Vec<usize>)> = vec![];
    for (i, t) in tracks.iter().enumerate() {
        let v = crate::data::data(t)["volumeNumber"].as_u64().unwrap_or(1);
        match out.iter_mut().find(|(n, _)| *n == v) {
            Some((_, ix)) => ix.push(i),
            None => out.push((v, vec![i])),
        }
    }
    (out.len() > 1).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn timestamps_read_their_zone() {
        assert_eq!(timestamp("1970-01-02"), Some(86400));
        assert_eq!(timestamp("1970-01-01T01:00:00.000+0000"), Some(3600));
        assert_eq!(timestamp("1970-01-01T01:00:00.000+0100"), Some(0));
        assert_eq!(timestamp("1970-01-01T01:00:00Z"), Some(3600));
        assert_eq!(timestamp("1970-01-01T00:00:00-02:00"), Some(7200));
        assert_eq!(timestamp("nonsense"), None);
    }

    #[test]
    fn dates_added_read_as_react() {
        let now = timestamp("2024-03-31T12:00:00Z").unwrap();
        assert_eq!(date_added("2024-03-30T12:00:00.000+0000", now), "This week");
        assert_eq!(date_added("2024-03-24T12:00:00.000+0000", now), "This week");
        assert_eq!(date_added("2024-03-20T12:00:00.000+0000", now), "Last week");
        assert_eq!(date_added("2024-03-05T12:00:00.000+0000", now), "Last month");
        assert_eq!(date_added("2024-01-01T00:00:00.000+0000", now), "Jan 1, 2024");
        assert_eq!(date_added("", now), "");
    }

    #[test]
    fn long_dates_durations_and_counts() {
        assert_eq!(release_long("2013-05-17"), "MAY 17, 2013");
        assert_eq!(total_duration(3725), "1:02:05");
        assert_eq!(total_duration(36), "0:36");
        assert_eq!(track_count(1, 0), "1 TRACK");
        assert_eq!(track_count(12, 2400), "12 TRACKS (40:00)");
    }

    #[test]
    fn compact_counts_as_intl() {
        assert_eq!(compact(0), "0");
        assert_eq!(compact(950), "950");
        assert_eq!(compact(1000), "1K");
        assert_eq!(compact(1234), "1.2K");
        assert_eq!(compact(12345), "12.3K");
        assert_eq!(compact(999_960), "1M");
        assert_eq!(compact(2_500_000), "2.5M");
        assert_eq!(fans(0), None);
        assert_eq!(fans(48210).as_deref(), Some("48.2K fans"));
    }

    #[test]
    fn badges_and_subtitles() {
        assert_eq!(quality_badge(&json!({"mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"]}})), Some(("HI-RES LOSSLESS", 2)));
        assert_eq!(quality_badge(&json!({"audioQuality": "LOSSLESS"})), Some(("LOSSLESS", 1)));
        assert_eq!(quality_badge(&json!({})), None);
        let own = json!({"releaseDate": "2026-02-01", "type": "EP", "artists": [{"id": 7, "name": "A"}]});
        assert_eq!(artist_album_subtitle(&own, 7).as_deref(), Some("2026 · EP"));
        let guest = json!({"releaseDate": "2026-02-01", "artists": [{"id": 9, "name": "Various Artists"}]});
        assert_eq!(artist_album_subtitle(&guest, 7).as_deref(), Some("Various Artists · 2026"));
        assert_eq!(artist_album_subtitle(&json!({}), 7), None);
    }

    #[test]
    fn volumes_group_multi_disc_albums() {
        let one = vec![json!({"volumeNumber": 1}), json!({})];
        assert_eq!(volumes(&one), None);
        let two = vec![json!({"volumeNumber": 1}), json!({"volumeNumber": 2}), json!({"volumeNumber": 1})];
        assert_eq!(volumes(&two), Some(vec![(1, vec![0, 2]), (2, vec![1])]));
    }
}
