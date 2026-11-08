//! Transport glyphs as vector shapes on a 24-unit grid, so every variant colours and sizes them.

use rune::{Color, El, LineCap, Path, Shape, canvas};

#[derive(Clone, Copy)]
pub enum G {
    Play,
    Pause,
    Next,
    Prev,

    Shuffle,
    Repeat,
    Heart,
    HeartFilled,
    List,
    Close,
    Speaker,
    Back,
    Search,
    PopOut,
    Library,
}

pub fn glyph(g: G, size: f32, c: Color) -> El {
    canvas(shape(g, c)).size(size, size).a11y_hidden()
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<[f32; 2]> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

fn heart() -> Path {
    Path::new()
        .move_to([12.0, 20.0])
        .cubic_to([5.5, 15.5], [2.5, 11.5], [3.5, 8.0])
        .cubic_to([4.6, 4.2], [9.6, 3.6], [12.0, 7.6])
        .cubic_to([14.4, 3.6], [19.4, 4.2], [20.5, 8.0])
        .cubic_to([21.5, 11.5], [18.5, 15.5], [12.0, 20.0])
        .close()
}

fn shape(g: G, c: Color) -> Shape {
    let s = Shape::new().view_box(24.0, 24.0);
    match g {
        G::Play => s.fill(vec![[7.0, 4.5], [20.0, 12.0], [7.0, 19.5]], c),
        G::Pause => s.fill(rect(6.0, 4.5, 10.0, 19.5), c).fill(rect(14.0, 4.5, 18.0, 19.5), c),
        G::Next => s.fill(vec![[4.5, 5.5], [15.5, 12.0], [4.5, 18.5]], c).fill(rect(16.5, 5.5, 19.5, 18.5), c),
        G::Prev => s.fill(vec![[19.5, 5.5], [8.5, 12.0], [19.5, 18.5]], c).fill(rect(4.5, 5.5, 7.5, 18.5), c),
        
        G::Shuffle => s
            .stroke(vec![[3.5, 7.0], [7.5, 7.0], [16.0, 17.0], [20.0, 17.0]], 1.8, c)
            .stroke(vec![[3.5, 17.0], [7.5, 17.0], [16.0, 7.0], [20.0, 7.0]], 1.8, c)
            .stroke(vec![[17.5, 4.5], [20.0, 7.0], [17.5, 9.5]], 1.8, c)
            .stroke(vec![[17.5, 14.5], [20.0, 17.0], [17.5, 19.5]], 1.8, c),
        G::Repeat => s
            .stroke(vec![[4.5, 12.0], [4.5, 8.5], [6.0, 7.0], [19.0, 7.0]], 1.8, c)
            .stroke(vec![[16.5, 4.5], [19.0, 7.0], [16.5, 9.5]], 1.8, c)
            .stroke(vec![[19.5, 12.0], [19.5, 15.5], [18.0, 17.0], [5.0, 17.0]], 1.8, c)
            .stroke(vec![[7.5, 14.5], [5.0, 17.0], [7.5, 19.5]], 1.8, c),
        G::Heart => s.stroke_path(&heart(), 1.8, LineCap::Round, c),
        G::HeartFilled => s.fill_path(&heart(), c),
        G::List => s
            .line([4.0, 7.0], [20.0, 7.0], 1.8, LineCap::Round, c)
            .line([4.0, 12.0], [20.0, 12.0], 1.8, LineCap::Round, c)
            .line([4.0, 17.0], [14.0, 17.0], 1.8, LineCap::Round, c),
        G::Close => s
            .line([6.0, 6.0], [18.0, 18.0], 1.8, LineCap::Round, c)
            .line([18.0, 6.0], [6.0, 18.0], 1.8, LineCap::Round, c),
        G::Speaker => s
            .fill(vec![[4.0, 9.5], [8.0, 9.5], [13.0, 5.0], [13.0, 19.0], [8.0, 14.5], [4.0, 14.5]], c)
            .stroke(vec![[16.0, 9.0], [17.0, 10.5], [17.2, 12.0], [17.0, 13.5], [16.0, 15.0]], 1.6, c),
        G::Back => s.stroke(vec![[15.0, 5.0], [8.0, 12.0], [15.0, 19.0]], 2.0, c),
        G::Search => s
            .stroke(
                (0..=24).map(|i| {
                    let a = i as f32 / 24.0 * std::f32::consts::TAU;
                    [10.5 + 6.0 * a.cos(), 10.5 + 6.0 * a.sin()]
                }).collect::<Vec<_>>(),
                1.9,
                c,
            )
            .line([15.0, 15.0], [20.0, 20.0], 1.9, LineCap::Round, c),
        // A window with a smaller window lifted out of its corner: picture-in-picture.
        G::PopOut => s
            .stroke(vec![[11.0, 18.5], [4.5, 18.5], [4.5, 5.5], [19.5, 5.5], [19.5, 11.0]], 1.7, c)
            .fill(rect(13.0, 13.5, 21.0, 20.0), c),
        G::Library => s
            .line([5.0, 5.0], [5.0, 19.0], 1.9, LineCap::Round, c)
            .line([9.5, 5.0], [9.5, 19.0], 1.9, LineCap::Round, c)
            .line([14.0, 5.5], [18.5, 18.5], 1.9, LineCap::Round, c),
    }
}
