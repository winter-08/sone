//! Skeleton placeholders while a page loads (React PageSkeleton: DetailPageSkeleton,
//! ArtistPageSkeleton, SearchSkeleton, grids and TrackList's row skeletons): grey blocks in
//! the shape of what is coming, so the page doesn't jump when it lands.
//!
//! They are still: React pulses them (animate-pulse), but a pulse here would redraw the window
//! every frame for as long as a fetch takes, which can be forever offline. They say they're
//! loading through the page's `busy` state (views::took_loading), not by name.

use rune::{El, col, el, row};

use crate::data::Key;
use crate::ui::white;

fn bone(w: f32, h: f32, r: f32) -> El {
    el().size(w, h).radius(r).fill(white(0.06))
}

/// A block `frac` of its row wide.
fn bar(frac: f32, h: f32) -> El {
    row().w_full().child(el().w_frac(frac).h(h).radius(h / 2.0).fill(white(0.06)))
}

/// `n` track rows (48 pt, as views::track_list's), with a cover when the list has them.
pub fn rows(n: usize, cover: bool) -> El {
    crate::views::note_loading();
    let mut c = col().w_full().gap(2.0).a11y_hidden();
    for _ in 0..n {
        let mut r = row().w_full().h(48.0).px(12.0).gap(12.0).items_center().child(bone(18.0, 12.0, 4.0));
        if cover {
            r = r.child(bone(40.0, 40.0, 4.0));
        }
        r = r.child(col().grow(1.0).min_w(0.0).gap(6.0).child(bar(0.45, 12.0)).child(bar(0.28, 10.0)))
            .child(el().w(220.0).shrink().child(bar(0.6, 10.0)))
            .child(bone(34.0, 10.0, 5.0));
        c = c.child(r);
    }
    c
}

/// A detail page's header: the art (round for an artist), title, subtitle and the actions.
fn hero(round: bool) -> El {
    row().w_full().gap(24.0).items_end()
        .child(bone(180.0, 180.0, if round { 90.0 } else { 14.0 }))
        .child(col().grow(1.0).gap(12.0).pb(4.0)
            .child(bar(0.12, 10.0))
            .child(bar(0.45, 34.0))
            .child(bar(0.25, 12.0))
            .child(row().gap(8.0).pt(6.0).child(bone(96.0, 40.0, 20.0)).child(bone(110.0, 40.0, 20.0)).child(bone(40.0, 40.0, 20.0)).child(bone(40.0, 40.0, 20.0))))
}

/// A row of cards (Home, Explore, an artist's discography).
fn cards(n: usize) -> El {
    let mut r = row().w_full().gap(8.0).flex_wrap();
    for _ in 0..n {
        r = r.child(col().w(148.0).gap(8.0).padding(8.0).child(bone(132.0, 132.0, 10.0)).child(bar(0.8, 12.0)).child(bar(0.5, 10.0)));
    }
    r
}

fn grid_page(sections: usize) -> El {
    let mut c = col().w_full().gap(22.0);
    for _ in 0..sections {
        c = c.child(col().w_full().gap(10.0).child(bar(0.2, 20.0)).child(cards(6)));
    }
    c
}

/// What shows while `key` loads.
pub fn page(key: &Key) -> El {
    crate::views::note_loading();
    let body = match key {
        Key::Album(_) | Key::Playlist(_) | Key::Mix(_) => col().w_full().gap(18.0).child(hero(false)).child(rows(10, !matches!(key, Key::Album(_)))),
        Key::Artist(_) => col().w_full().gap(18.0).child(hero(true)).child(bar(0.15, 20.0)).child(rows(5, true)).child(bar(0.15, 20.0)).child(cards(6)),
        Key::Search(_) => {
            let mut tabs = row().gap(6.0);
            for w in [96.0, 78.0, 64.0, 64.0, 78.0, 64.0, 64.0] {
                tabs = tabs.child(bone(w, 30.0, 15.0));
            }
            col().w_full().gap(16.0).child(bar(0.35, 30.0)).child(tabs).child(bar(0.12, 20.0)).child(rows(8, true)).child(bar(0.12, 20.0)).child(cards(6))
        }
        Key::Home(_) | Key::Page(_) => grid_page(3),
        _ => return crate::views::loading(),
    };
    body.a11y_hidden()
}
