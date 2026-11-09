//! TIDAL's editorial text (artist biographies, album reviews): plain text with `<br/>` line
//! breaks, a few HTML entities and `[wimpLink artistId="…"]Name[/wimpLink]` cross-references
//! (also `albumId`). React's BioText renders artist links as buttons and drops every other
//! tag; this does the same with Rune's in-text links, and links albums too.

use rune::{El, Link, LinkPaint, Span};

use crate::ui::{t, white};
use crate::views::{go, Page};
use crate::App;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Artist(u64),
    Album(u64),
}

impl Target {
    fn id(self) -> String {
        match self {
            Target::Artist(i) => format!("artist:{i}"),
            Target::Album(i) => format!("album:{i}"),
        }
    }
    fn parse(id: &str) -> Option<Target> {
        let (k, n) = id.split_once(':')?;
        let n = n.parse().ok()?;
        match k {
            "artist" => Some(Target::Artist(n)),
            "album" => Some(Target::Album(n)),
            _ => None,
        }
    }
}

/// One paragraph: its text and the byte ranges that link somewhere.
#[derive(Debug, PartialEq)]
pub struct Para {
    pub text: String,
    pub links: Vec<(std::ops::Range<usize>, Target)>,
}

fn entities(s: &str) -> String {
    s.replace("&nbsp;", " ").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'")
        .replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// `<br>`, `<br/>`, `<br />` in any case become newlines; every other `<…>` tag is dropped.
fn html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else { out.push_str(&rest[i..]); return out };
        let tag = rest[i + 1..i + j].trim().trim_end_matches('/').trim().to_ascii_lowercase();
        if tag == "br" || tag == "p" || tag == "/p" {
            out.push('\n');
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    out
}

/// The id a `[wimpLink …]` opener names, if it is one we can open.
fn target_of(tag: &str) -> Option<Target> {
    let attr = |name: &str| -> Option<u64> {
        let at = tag.find(&format!("{name}=\""))? + name.len() + 2;
        tag[at..].split('"').next()?.parse().ok()
    };
    attr("artistId").map(Target::Artist).or_else(|| attr("albumId").map(Target::Album))
}

fn paragraph(src: &str) -> Para {
    let mut p = Para { text: String::new(), links: vec![] };
    let mut rest = src;
    let mut open: Option<(usize, Option<Target>)> = None;
    while let Some(i) = rest.find('[') {
        p.text.push_str(&entities(&rest[..i]));
        let Some(j) = rest[i..].find(']') else { p.text.push_str(&entities(&rest[i..])); rest = ""; break };
        let tag = &rest[i + 1..i + j];
        if tag.starts_with("wimpLink") {
            open = Some((p.text.len(), target_of(tag)));
        } else if tag == "/wimpLink" {
            if let Some((start, Some(to))) = open.take() {
                // Keep the link off surrounding spaces so the underline hugs the name.
                let s = &p.text[start..];
                let a = start + (s.len() - s.trim_start().len());
                let b = p.text.trim_end().len().max(a);
                if b > a {
                    p.links.push((a..b, to));
                }
            }
        }
        rest = &rest[i + j + 1..];
    }
    p.text.push_str(&entities(rest));
    // Trim the paragraph, shifting the link ranges with it.
    let lead = p.text.len() - p.text.trim_start().len();
    p.text = p.text.trim().to_string();
    let len = p.text.len();
    for (r, _) in &mut p.links {
        *r = r.start.saturating_sub(lead).min(len)..r.end.saturating_sub(lead).min(len);
    }
    p.links.retain(|(r, _)| r.start < r.end);
    p
}

/// The paragraphs of `raw`, blank ones dropped (React splits on every newline).
pub fn parse(raw: &str) -> Vec<Para> {
    html(raw).split('\n').filter(|l| !l.trim().is_empty()).map(paragraph).collect()
}

/// All of it as one plain line, for the clamped preview under an artist's name.
pub fn plain(raw: &str) -> String {
    parse(raw).into_iter().map(|p| p.text).collect::<Vec<_>>().join(" ")
}

fn open(s: &mut App, id: &str) {
    let Some(to) = Target::parse(id) else { return };
    s.nav.dialog = None;
    go(s, match to {
        Target::Artist(i) => Page::Artist(i),
        Target::Album(i) => Page::Album(i),
    });
}

/// The text as paragraphs, each link underlined faintly in the text colour and fully on
/// hover (React BioText: `underline decoration-current/40`, `/80` on hover).
pub fn view(raw: &str, size: f32, alpha: f32) -> El {
    let c = white(alpha);
    let mut col = rune::col().w_full().gap(size * 0.9);
    for p in parse(raw) {
        let spans: Vec<Span> = p.links.iter().map(|(r, _)| Span::new(r.start, r.end).underline(white(alpha * 0.45))).collect();
        let mut e = rune::rich(p.text.clone(), spans).font(rune::TextStyle { line_height: size * 1.7, ..t(size, 400.0) }).color(c);
        for (r, to) in &p.links {
            e = e.link(Link::new(r.clone(), to.id()).hover(LinkPaint::new().color(white(1.0)).underline(white(0.9))));
        }
        if !p.links.is_empty() {
            e = e.on_link(|s: &mut App, id: &str, _tx: &mut rune::Tx| open(s, id));
        }
        col = col.child(e);
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_breaks_and_entities() {
        let raw = r#"Formed with [wimpLink artistId="42"]Jane Doe[/wimpLink] &amp; friends.<br/><br/>Their debut [wimpLink albumId="7"] Blue [/wimpLink] [b]sold[/b] well.<br />[wimpLink trackId="1"]Song[/wimpLink] too."#;
        let ps = parse(raw);
        assert_eq!(ps.len(), 3);
        assert_eq!(ps[0].text, "Formed with Jane Doe & friends.");
        assert_eq!(&ps[0].text[ps[0].links[0].0.clone()], "Jane Doe");
        assert_eq!(ps[0].links[0].1, Target::Artist(42));
        assert_eq!(ps[1].text, "Their debut  Blue  sold well.");
        assert_eq!(&ps[1].text[ps[1].links[0].0.clone()], "Blue");
        assert_eq!(ps[1].links[0].1, Target::Album(7));
        assert_eq!(ps[2].text, "Song too.");
        assert!(ps[2].links.is_empty());
        assert_eq!(plain(raw), "Formed with Jane Doe & friends. Their debut  Blue  sold well. Song too.");
    }

    #[test]
    fn leading_link_survives_trim() {
        let ps = parse(r#"  [wimpLink artistId="5"]Ann[/wimpLink] sings."#);
        assert_eq!(ps[0].text, "Ann sings.");
        assert_eq!(&ps[0].text[ps[0].links[0].0.clone()], "Ann");
    }
}
