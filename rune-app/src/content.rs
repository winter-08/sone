//! The explicit / AI-generated content filter and the badges that mark such tracks (React
//! src/lib/contentFilter.ts, ExplicitBadge, AiBadge, src/lib/trackAvailability.ts).
//!
//! The two switches are the React `sone.allowExplicit.v1` / `sone.allowAi.v1` preferences: the
//! controller owns them (persisted with its other preferences) and mirrors them here, so a view
//! can dim a blocked row without asking it. Both default to allowed.

use crate::theme;
use crate::ui::{t, white};
use rune::{El, text};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

static ALLOW_EXPLICIT: AtomicBool = AtomicBool::new(true);
static ALLOW_AI: AtomicBool = AtomicBool::new(true);

/// Why a track is filtered out (React BlockReason).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Ai,
    Explicit,
}

/// The switches as the controller last set them: (allow explicit, allow AI).
pub fn prefs() -> (bool, bool) {
    (ALLOW_EXPLICIT.load(Ordering::Relaxed), ALLOW_AI.load(Ordering::Relaxed))
}

/// From the controller, at start-up and whenever either switch changes.
pub(crate) fn set_prefs(allow_explicit: bool, allow_ai: bool) {
    ALLOW_EXPLICIT.store(allow_explicit, Ordering::Relaxed);
    ALLOW_AI.store(allow_ai, Ordering::Relaxed);
}

/// contentBlockReason: AI first, then explicit. `t` may be a track or a catalogue item wrapping
/// one (`{item: …}` or `{data: …}`), as the views hold them.
pub fn reason_with(t: &Value, allow_explicit: bool, allow_ai: bool) -> Option<Block> {
    let t = inner(t);
    if !allow_ai && t["ai"].as_bool() == Some(true) {
        return Some(Block::Ai);
    }
    if !allow_explicit && t["explicit"].as_bool() == Some(true) {
        return Some(Block::Explicit);
    }
    None
}

/// The track inside a catalogue wrapper (`{data}`, `{value}`, `{item}`), or `t` itself.
fn inner(t: &Value) -> &Value {
    let t = crate::data::data(t);
    if t.get("item").is_some_and(Value::is_object) { &t["item"] } else { t }
}

/// [`reason_with`] under the current switches.
pub fn reason(t: &Value) -> Option<Block> {
    let (e, a) = prefs();
    reason_with(t, e, a)
}

pub fn blocked(t: &Value) -> bool {
    reason(t).is_some()
}

/// blockedMessage: the info toast a refused play shows.
pub fn message(b: Block) -> &'static str {
    match b {
        Block::Ai => "AI content is turned off in Settings",
        Block::Explicit => "Explicit content is turned off in Settings",
    }
}

/// isTrackUnavailable: not streamable now (`streamReady` or `allowStreaming` false, or a
/// `streamStartDate` still to come). Videos aren't gated; missing fields mean available.
pub fn unavailable(t: &Value) -> bool {
    if t["itemType"].as_str() == Some("video") {
        return false;
    }
    if t["streamReady"].as_bool() == Some(false) || t["allowStreaming"].as_bool() == Some(false) {
        return true;
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    t["streamStartDate"].as_str().and_then(parse_date).is_some_and(|at| at > now)
}

/// Seconds since the epoch for an ISO 8601 date or date-time as TIDAL writes them
/// ("2031-01-01", "2031-01-01T00:00:00.000+0000", "...Z", "...+01:00"). None if unreadable,
/// which (as Date.parse's NaN in React) counts as available.
pub fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    let num = |a: usize, b: usize| s.get(a..b).and_then(|x| x.parse::<i64>().ok());
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    if s.as_bytes().get(4) != Some(&b'-') || s.as_bytes().get(7) != Some(&b'-') || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    // days_from_civil (Howard Hinnant)
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let mut secs = days * 86400;
    if s.len() > 10 {
        if !matches!(s.as_bytes()[10], b'T' | b' ') {
            return None;
        }
        let (h, mi) = (num(11, 13)?, num(14, 16)?);
        let sec = num(17, 19).unwrap_or(0);
        secs += h * 3600 + mi * 60 + sec;
        // The zone: Z, or ±hh[:]mm after the time (and any fraction).
        let rest = &s[16..];
        if let Some(i) = rest.find(['+', '-']) {
            let z = rest[i + 1..].replace(':', "");
            let (zh, zm) = (z.get(0..2).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0), z.get(2..4).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0));
            let off = zh * 3600 + zm * 60;
            secs -= if rest.as_bytes()[i] == b'+' { off } else { -off };
        }
    }
    Some(secs)
}

/// ExplicitBadge: a small "E" plate after a title. Decorative here (the React span has no name).
pub fn explicit_badge() -> El {
    rune::el().size(15.0, 15.0).radius(3.0).fill(white(0.12)).items_center().justify_center().a11y_hidden()
        .child(text("E").font(t(9.0, 700.0)).color(white(theme::TEXT_MUTED)))
}

/// AiBadge: "AI", named "AI-generated" as the React badge is.
pub fn ai_badge() -> El {
    rune::el().h(15.0).px(4.0).radius(3.0).fill(white(0.12)).items_center().justify_center()
        .child(text("AI").font(t(9.0, 700.0).tracking(0.6)).color(white(theme::TEXT_MUTED)).label("AI-generated"))
}

/// The badges a track's title carries, in React's order (explicit, then AI); None for neither.
pub fn badges(explicit: bool, ai: bool) -> Option<El> {
    if !explicit && !ai {
        return None;
    }
    let mut r = rune::row().gap(4.0).items_center();
    if explicit {
        r = r.child(explicit_badge());
    }
    if ai {
        r = r.child(ai_badge());
    }
    Some(r)
}

/// [`badges`] for a track or catalogue item's own flags.
pub fn badges_for(t: &Value) -> Option<El> {
    let d = inner(t);
    badges(d["explicit"].as_bool() == Some(true), d["ai"].as_bool() == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ai_wins_over_explicit_and_missing_flags_pass() {
        let both = json!({"explicit": true, "ai": true});
        assert_eq!(reason_with(&both, false, false), Some(Block::Ai));
        assert_eq!(reason_with(&both, false, true), Some(Block::Explicit));
        assert_eq!(reason_with(&both, true, true), None);
        assert_eq!(reason_with(&json!({"id": 1}), false, false), None);
        assert_eq!(reason_with(&json!({"item": {"explicit": true}}), false, true), Some(Block::Explicit));
    }

    #[test]
    fn reads_tidal_dates() {
        assert_eq!(parse_date("1970-01-02"), Some(86400));
        assert_eq!(parse_date("2000-03-01T00:00:00.000+0000"), Some(951868800));
        assert_eq!(parse_date("2000-03-01T01:00:00+01:00"), Some(951868800));
        assert_eq!(parse_date("2000-03-01T00:00:00Z"), Some(951868800));
        assert_eq!(parse_date("soon"), None);
    }

    #[test]
    fn a_future_start_date_is_unavailable() {
        assert!(unavailable(&json!({"streamStartDate": "2999-01-01T00:00:00.000+0000"})));
        assert!(!unavailable(&json!({"streamStartDate": "2001-01-01T00:00:00.000+0000"})));
        assert!(!unavailable(&json!({"streamStartDate": "garbage"})));
        assert!(unavailable(&json!({"allowStreaming": false})));
        assert!(!unavailable(&json!({"itemType": "video", "streamReady": false})));
    }
}
