//! The drawer's Suggested tracks and Credits tabs (React NowPlayingDrawer SuggestedTab /
//! SuggestedTrackRow and CreditsTab).

use crate::App;
use crate::controller::{self, Cmd, View};
use crate::data::{self, Key};
use crate::glyph::{G, glyph};
use crate::interact::{self, Anchor, Fav, Say};
use crate::theme::{self, accent};
use crate::ui::{self, caps, t, white};
use crate::views::{self, Page};
use rune::{El, col, el, row, text};
use serde_json::Value;

/// A quiet centred state with an icon, as the React tabs' empty states.
fn empty(g: G, msg: &str) -> El {
    col().w_full().py(64.0).gap(12.0).items_center()
        .child(glyph(g, 40.0, white(0.25)))
        .child(text(msg.to_string()).font(t(13.0, 500.0)).color(white(0.4)))
}

fn hover_text(e: El) -> El {
    el().shrink().min_w(0.0).hover_group().child(e.group_color(white(0.92)))
}

/// The suggested tracks: the current track's radio (its TRACK_MIX).
pub fn suggested(v: &View) -> El {
    let none = || empty(G::Sparkles, "No suggested tracks available for this track");
    let Some(mix) = &v.track_mix else { return none() };
    let m = match data::want(Key::Mix(mix.clone())) {
        data::Load::Ready(m) => m,
        data::Load::Failed(_) => return none(),
        data::Load::Loading => return views::loading(),
    };
    let tracks = m["tracks"].as_array().cloned().unwrap_or_default();
    if tracks.is_empty() {
        return none();
    }
    let mut c = col().w_full().gap(2.0);
    for (i, tr) in tracks.iter().enumerate() {
        c = c.child(suggested_row(v, tr, i));
    }
    c
}

/// `Added "<title>" to queue`, the title cut to 28 characters past 30 (SuggestedTab).
pub fn added_toast(title: &str) -> String {
    let label = if title.chars().count() > 30 { title.chars().take(28).collect::<String>() + "…" } else { title.to_string() };
    format!("Added \"{label}\" to queue")
}

/// SuggestedTrackRow: a press queues it (tagged with its album as the source it plays from);
/// the cover plays it now. A track the content filter blocks dims and refuses both.
fn suggested_row(v: &View, tr: &Value, i: usize) -> El {
    let key = format!("sg-{i}");
    let id = tr["id"].as_u64().unwrap_or(0);
    let active = v.track_id == Some(id);
    let blocked = crate::content::blocked(tr);
    let title = match tr["version"].as_str().filter(|s| !s.is_empty()) {
        Some(ver) => format!("{} ({ver})", tr["title"].as_str().unwrap_or("")),
        None => tr["title"].as_str().unwrap_or("").to_string(),
    };
    let cover_id = tr["album"]["cover"].as_str().map(str::to_string);
    let mut cover = ui::with_image(ui::tile(ui::hue(Some(id)), 40.0, id, 4.0), crate::art::cover(cover_id.as_deref(), 40.0), 40.0, 4.0);
    if !blocked {
        let t_play = tr.clone();
        // the play mark shows over the cover while the pointer is on it
        cover = theme::focusable(el().key(format!("{key}-cover")).role("button").label(format!("Play {title}"))).radius(4.0).hover_group().cursor(rune::Cursor::Pointer)
            .on_press(move |_: &mut App| controller::send(Cmd::PlayTrack(t_play.clone())))
            .child(cover)
            .child(row().absolute(0.0, 0.0).size(40.0, 40.0).radius(4.0).fill(theme::black(0.4)).items_center().justify_center().group_opacity(0.0, 1.0).a11y_hidden()
                .child(glyph(G::Play, 14.0, white(1.0))));
    }
    let mut head = row().gap(6.0).min_w(0.0).items_center()
        .child(text(title.clone()).font(t(13.0, 500.0)).color(if active { accent() } else { white(theme::TEXT) }).truncate().shrink());
    if let Some(b) = crate::content::badges_for(tr) {
        head = head.child(b);
    }
    let mut sub = row().min_w(0.0).items_center().child(crate::queue_ui::row_artists(&key, tr));
    if let Some(album) = tr["album"]["title"].as_str().filter(|a| !a.is_empty()) {
        sub = sub.child(text(" · ").font(t(11.0, 400.0)).color(white(0.5)).a11y_hidden());
        let a = text(album.to_string()).font(t(11.0, 400.0)).color(white(0.5)).truncate().shrink();
        sub = match tr["album"]["id"].as_u64() {
            Some(alb) => sub.child(hover_text(a).cursor(rune::Cursor::Pointer).on_press(move |s: &mut App| views::go(s, Page::Album(alb)))),
            None => sub.child(a),
        };
    }
    let loved = interact::is_fav(Fav::Track, &id.to_string());
    let (more_key, t_more, t_menu, t_queue, t_row) = (format!("{key}-more"), tr.clone(), tr.clone(), tr.clone(), tr.clone());
    let mk = more_key.clone();
    let (fl, ql, rl) = (title.clone(), title.clone(), title);
    let queue = move |t: &Value, label: &str| {
        controller::send(Cmd::QueueTrack(t.clone()));
        interact::toast(added_toast(label), interact::Kind::Success);
    };
    let actions = row().gap(2.0).items_center()
        .child(ui::icon_button::<App>(&more_key, "More options", G::More, 15.0, white(0.45))
            .on_press(move |s: &mut App| interact::open_track(s, t_more.clone(), None, Anchor::Below(mk.clone()))))
        .child(ui::icon_button::<App>(&format!("{key}-love"), if loved { "Remove from Loved tracks" } else { "Add to Loved tracks" }, if loved { G::HeartFilled } else { G::Heart }, 15.0, if loved { accent() } else { white(0.45) })
            .on_press(move |_: &mut App| interact::toggle_fav(Fav::Track, id.to_string(), fl.clone(), Say::LovedShort)))
        .child(ui::icon_button::<App>(&format!("{key}-queue"), "Add to queue", G::ListPlus, 15.0, white(0.45))
            .on_press(move |_: &mut App| if !blocked { queue(&t_queue, &ql) }));
    let mut r = row().key(key).role("presentation").w_full().h(56.0).px(10.0).gap(12.0).radius(8.0).items_center()
        .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, t_menu.clone(), None, Anchor::Point(p.x, p.y)))
        .child(cover)
        .child(col().grow(1.0).min_w(0.0).gap(2.0).child(head).child(sub))
        .child(actions);
    if blocked {
        r = r.opacity(0.4);
    } else {
        r = theme::focusable_row(r).fill(if active { white(0.08) } else { white(0.0) }).hover(white(0.05)).cursor(rune::Cursor::Pointer)
            .on_press(move |_: &mut App| {
                controller::send(Cmd::QueueTrack(t_row.clone()));
                interact::toast(added_toast(&rl), interact::Kind::Success);
            });
    }
    r
}

/// "M/D/YYYY" for an ISO date (CreditsTab's toLocale-free `Released`).
fn released(date: &str) -> Option<String> {
    let (y, rest) = date.split_once('-')?;
    let (m, rest) = rest.split_once('-')?;
    let d = rest.get(0..2)?;
    Some(format!("{}/{}/{}", m.parse::<u32>().ok()?, d.parse::<u32>().ok()?, y))
}

fn credit_row(label: &str, value: String, first: bool) -> El {
    let mut r = col().w_full().gap(4.0).py(14.0)
        .child(caps(label, 0.45))
        .child(text(value).font(t(15.0, 500.0).lh(22.0)).color(white(theme::TEXT)).wrap());
    if !first {
        r = r.border_sides([1.0, 0.0, 0.0, 0.0], white(0.06));
    }
    r
}

/// The credits: what is playing (title, artists, album, release date, label), the contributors
/// by role, then the main artist's biography with its links.
pub fn credits(v: &View) -> El {
    let Some(cur) = &v.current else { return empty(G::Users, "No credits available for this track") };
    let tr = &*cur.track;
    let credits = data::want(Key::Credits(cur.id));
    let bio = v.artist_id.map(|aid| data::want(Key::ArtistBio(aid)));
    let bio_text = match &bio {
        Some(data::Load::Ready(b)) => b["text"].as_str().or(b.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string),
        _ => None,
    };
    let bio_loading = matches!(bio, Some(data::Load::Loading));
    let roles: Vec<(String, String)> = match &credits {
        data::Load::Ready(cr) => cr.as_array().cloned().unwrap_or_default().iter().map(|c| {
            let role = c["type"].as_str().or(c["creditType"].as_str()).unwrap_or("").to_string();
            let names: Vec<&str> = c["contributors"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect()).unwrap_or_default();
            (role, names.join(", "))
        }).collect(),
        _ => vec![],
    };
    let credits_loading = matches!(credits, data::Load::Loading);
    if !credits_loading && roles.is_empty() && !bio_loading && bio_text.is_none() {
        return empty(G::Users, "No credits available for this track");
    }
    let mut c = col().w_full()
        .child(credit_row("Title", cur.title.clone(), true))
        .child(credit_row("Artists", cur.artist.clone(), false));
    if !cur.album.is_empty() {
        c = c.child(credit_row("Album", cur.album.clone(), false));
    }
    if let Some(d) = tr["album"]["releaseDate"].as_str().and_then(released) {
        c = c.child(credit_row("Released", d, false));
    }
    if let Some(l) = tr["copyright"].as_str().filter(|l| !l.is_empty()) {
        c = c.child(credit_row("Label", l.to_string(), false));
    }
    if credits_loading {
        c = c.child(col().py(14.0).child(views::loading()));
    }
    for (role, names) in roles {
        c = c.child(credit_row(&role, names, false));
    }
    if bio_loading {
        c = c.child(col().pt(24.0).child(views::loading()));
    }
    if let Some(b) = bio_text {
        c = c.child(col().w_full().pt(24.0).gap(12.0).child(views::heading("Bio", 16.0)).child(crate::bio::view(&b, 13.0, 0.7)));
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_titles_are_cut_in_the_queue_toast() {
        assert_eq!(added_toast("Short"), "Added \"Short\" to queue");
        let long = "A".repeat(31);
        assert_eq!(added_toast(&long), format!("Added \"{}…\" to queue", "A".repeat(28)));
    }

    #[test]
    fn release_dates_read_month_first() {
        assert_eq!(released("2021-03-07").as_deref(), Some("3/7/2021"));
        assert_eq!(released("2021-12-25T00:00:00.000+0000").as_deref(), Some("12/25/2021"));
        assert_eq!(released("soon"), None);
    }
}
