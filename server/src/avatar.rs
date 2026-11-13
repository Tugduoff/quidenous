//! Drawn avatars. An avatar is a spec: one option index per field of `assets/avatar.json`, dot-separated,
//! in the order of its `fields` (`"0.2.5.1.3.1.2.0.0.4.6"`). That JSON is the single source of truth for
//! the app (TypeScript renderer, a byte-identical copy in `app/src/lib/avatar-parts.json`) and for the
//! share cards here: option counts always come from it, never from the code.

use std::{collections::HashMap, fmt::Write, sync::OnceLock};

use serde::Deserialize;

/// Longest spec accepted (11 fields of 2 digits and their dots is 32).
const SPEC_MAX: usize = 48;

#[derive(Deserialize)]
struct Def {
    fields: Vec<String>,
    /// Always drawn (neck, shirt).
    #[serde(default)]
    base: Vec<Shape>,
    #[serde(default)]
    presets: Vec<String>,
    /// Palette fields (`skin`, `hairColor`, `bg`): each option is `[colour, colour]`.
    /// Part fields (`face`, `hair`, …): each option is a list of shapes (may be empty).
    #[serde(flatten)]
    options: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Default)]
struct Shape {
    el: String,
    d: Option<String>,
    cx: Option<f64>,
    cy: Option<f64>,
    r: Option<f64>,
    rx: Option<f64>,
    ry: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    w: Option<f64>,
    h: Option<f64>,
    fill: Option<String>,
    stroke: Option<String>,
    sw: Option<f64>,
    op: Option<f64>,
    layer: Option<String>,
}

impl Shape {
    fn back(&self) -> bool {
        self.layer.as_deref() == Some("back")
    }
}

/// The parsed parts, per field name.
struct Parts {
    fields: Vec<String>,
    base: Vec<Shape>,
    presets: Vec<String>,
    palettes: HashMap<String, Vec<Vec<String>>>,
    shapes: HashMap<String, Vec<Vec<Shape>>>,
}

const PALETTES: [&str; 3] = ["skin", "hairColor", "bg"];

fn parts() -> &'static Parts {
    static PARTS: OnceLock<Parts> = OnceLock::new();
    PARTS.get_or_init(|| {
        let def: Def = serde_json::from_str(include_str!("../assets/avatar.json"))
            .expect("assets/avatar.json is invalid");
        let mut palettes = HashMap::new();
        let mut shapes = HashMap::new();
        for field in &def.fields {
            let Some(value) = def.options.get(field) else {
                continue;
            };
            if PALETTES.contains(&field.as_str()) {
                let p: Vec<Vec<String>> = serde_json::from_value(value.clone())
                    .unwrap_or_else(|e| panic!("assets/avatar.json: palette {field}: {e}"));
                palettes.insert(field.clone(), p);
            } else {
                let s: Vec<Vec<Shape>> = serde_json::from_value(value.clone())
                    .unwrap_or_else(|e| panic!("assets/avatar.json: part {field}: {e}"));
                shapes.insert(field.clone(), s);
            }
        }
        Parts {
            fields: def.fields,
            base: def.base,
            presets: def.presets,
            palettes,
            shapes,
        }
    })
}

impl Parts {
    /// How many options a field has (0 for a field the JSON does not define).
    fn count(&self, field: &str) -> usize {
        self.palettes
            .get(field)
            .map(Vec::len)
            .or_else(|| self.shapes.get(field).map(Vec::len))
            .unwrap_or(0)
    }

    /// The option index of every field, or None if the spec is not a valid one.
    fn parse(&self, spec: &str) -> Option<HashMap<&str, usize>> {
        if spec.is_empty() || spec.len() > SPEC_MAX {
            return None;
        }
        let items: Vec<&str> = spec.split('.').collect();
        if items.len() != self.fields.len() {
            return None;
        }
        let mut out = HashMap::new();
        for (field, item) in self.fields.iter().zip(items) {
            // Canonical decimal only: "0", "7", "12"; not "", "01", "+1", " 1".
            if item.is_empty()
                || item.len() > 2
                || !item.bytes().all(|b| b.is_ascii_digit())
                || (item.len() > 1 && item.starts_with('0'))
            {
                return None;
            }
            let i: usize = item.parse().ok()?;
            if i >= self.count(field) {
                return None;
            }
            out.insert(field.as_str(), i);
        }
        Some(out)
    }
}

/// Is this a spec the current parts can draw?
pub fn valid(spec: &str) -> bool {
    parts().parse(spec).is_some()
}

/// Ready-made avatars from the JSON.
pub fn presets() -> &'static [String] {
    &parts().presets
}

/// Numbers as short as they can be: `50`, `2.5`, `0.333`.
fn num(x: f64) -> String {
    let r = (x * 1000.0).round() / 1000.0;
    if r == 0.0 { "0".into() } else { format!("{r}") }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The colours a spec picked, by token.
struct Colors<'a> {
    skin: &'a [String],
    hair: &'a [String],
    bg: &'a [String],
}

impl Colors<'_> {
    /// A token, a literal `#RGB`/`#RRGGBB`(`AA`) or `none`; None for anything else.
    fn resolve(&self, c: &str) -> Option<String> {
        let pick = |p: &[String], i: usize| p.get(i).cloned();
        let out = match c {
            "skin" => pick(self.skin, 0),
            "skinShade" => pick(self.skin, 1),
            "hair" => pick(self.hair, 0),
            "hairShade" => pick(self.hair, 1),
            "bg" => pick(self.bg, 0),
            "shirt" => pick(self.bg, 1),
            "none" => Some("none".into()),
            _ => Some(c.to_owned()),
        }?;
        let hex = out.strip_prefix('#').unwrap_or("");
        let ok = out == "none"
            || (matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit()));
        ok.then_some(out)
    }
}

fn shape(out: &mut String, s: &Shape, colors: &Colors) {
    let mut attrs = String::new();
    let mut put = |name: &str, v: Option<f64>| {
        if let Some(v) = v {
            let _ = write!(attrs, r#" {name}="{}""#, num(v));
        }
    };
    let tag = match s.el.as_str() {
        "path" => "path",
        "circle" => {
            put("cx", s.cx);
            put("cy", s.cy);
            put("r", s.r);
            "circle"
        }
        "ellipse" => {
            put("cx", s.cx);
            put("cy", s.cy);
            put("rx", s.rx);
            put("ry", s.ry);
            "ellipse"
        }
        "rect" => {
            put("x", s.x);
            put("y", s.y);
            put("width", s.w);
            put("height", s.h);
            put("rx", s.rx);
            "rect"
        }
        _ => return,
    };
    if tag == "path" {
        let Some(d) = &s.d else { return };
        let _ = write!(attrs, r#" d="{}""#, esc(d));
    }
    let fill = s
        .fill
        .as_deref()
        .and_then(|c| colors.resolve(c))
        .unwrap_or_else(|| "none".into());
    let _ = write!(attrs, r#" fill="{fill}""#);
    if let Some(stroke) = s.stroke.as_deref().and_then(|c| colors.resolve(c)) {
        let _ = write!(
            attrs,
            r#" stroke="{stroke}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#,
            num(s.sw.unwrap_or(2.5))
        );
    }
    if let Some(op) = s.op {
        let _ = write!(attrs, r#" opacity="{}""#, num(op));
    }
    let _ = write!(out, "<{tag}{attrs}/>");
}

/// The avatar as an SVG fragment in a 0 0 100 100 space, clipped to the circle (50, 50, r 50), with a
/// clipPath of id `id` (unique in the document). Place it with a transform. None if the spec is invalid.
///
/// Draw order (the app's renderer follows exactly the same):
/// 1. the background (`bg`),
/// 2. the `layer: "back"` shapes of the hair option, then of the extra option,
/// 3. `base`,
/// 4. face, beard, mouth, eyes, brows, glasses (all their shapes, `back` or not),
/// 5. the other shapes of hair, then of extra.
pub fn svg(spec: &str, id: &str) -> Option<String> {
    let p = parts();
    let sel = p.parse(spec)?;
    let palette = |field: &str| -> &[String] {
        sel.get(field)
            .and_then(|&i| p.palettes.get(field)?.get(i))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    };
    let colors = Colors {
        skin: palette("skin"),
        hair: palette("hairColor"),
        bg: palette("bg"),
    };
    let option = |field: &str| -> &[Shape] {
        sel.get(field)
            .and_then(|&i| p.shapes.get(field)?.get(i))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    };
    let id = esc(id);
    let mut out = format!(
        r#"<g><defs><clipPath id="{id}"><circle cx="50" cy="50" r="50"/></clipPath></defs><g clip-path="url(#{id})">"#
    );
    // 1. Background.
    let bg = Shape {
        el: "rect".into(),
        x: Some(0.0),
        y: Some(0.0),
        w: Some(100.0),
        h: Some(100.0),
        fill: Some("bg".into()),
        ..Default::default()
    };
    shape(&mut out, &bg, &colors);
    // 2. Behind the head: long hair, hoods…
    for field in ["hair", "extra"] {
        for s in option(field).iter().filter(|s| s.back()) {
            shape(&mut out, s, &colors);
        }
    }
    // 3. Neck and shirt.
    for s in &p.base {
        shape(&mut out, s, &colors);
    }
    // 4. The face and what is on it.
    for field in ["face", "beard", "mouth", "eyes", "brows", "glasses"] {
        for s in option(field) {
            shape(&mut out, s, &colors);
        }
    }
    // 5. Hair and extras in front.
    for field in ["hair", "extra"] {
        for s in option(field).iter().filter(|s| !s.back()) {
            shape(&mut out, s, &colors);
        }
    }
    out.push_str("</g></g>");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_with(field: usize, value: &str) -> String {
        let p = parts();
        (0..p.fields.len())
            .map(|i| if i == field { value.to_owned() } else { "0".into() })
            .collect::<Vec<_>>()
            .join(".")
    }

    fn parses(fragment: &str) {
        let doc = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">{fragment}</svg>"#
        );
        let tree = resvg::usvg::Tree::from_str(&doc, &resvg::usvg::Options::default())
            .unwrap_or_else(|e| panic!("{e}: {doc}"));
        let mut pixmap = resvg::tiny_skia::Pixmap::new(100, 100).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );
        // The background fills the circle: its centre is never transparent.
        assert_ne!(pixmap.pixel(50, 50).unwrap().alpha(), 0, "{doc}");
    }

    #[test]
    fn the_json_defines_every_field() {
        let p = parts();
        assert!(!p.fields.is_empty());
        for f in &p.fields {
            assert!(p.count(f) > 0, "field {f} has no options");
            assert!(p.count(f) <= 100, "field {f}: indexes are 2 digits at most");
        }
        for f in PALETTES {
            for (i, pair) in p.palettes[f].iter().enumerate() {
                assert_eq!(pair.len(), 2, "{f}[{i}] needs two colours");
            }
        }
        let all = spec_with(usize::MAX, "");
        assert!(all.len() <= SPEC_MAX);
    }

    #[test]
    fn validation() {
        let p = parts();
        let zero = spec_with(usize::MAX, "");
        assert!(valid(&zero));
        for bad in ["", ".", "0", "abc", "-1", " 0"] {
            assert!(!valid(bad), "{bad:?}");
        }
        // Wrong field count.
        assert!(!valid(&format!("{zero}.0")));
        assert!(!valid(zero.strip_suffix(".0").unwrap()));
        assert!(!valid(&format!("{zero}.")));
        for (i, f) in p.fields.iter().enumerate() {
            let n = p.count(f);
            assert!(valid(&spec_with(i, &(n - 1).to_string())), "{f}");
            assert!(!valid(&spec_with(i, &n.to_string())), "{f} out of range");
            for bad in ["", "01", "00", "+0", "x", "0x", "100", " 0", "0 "] {
                assert!(!valid(&spec_with(i, bad)), "{f} = {bad:?}");
            }
        }
        // Too long, even if it were well formed.
        assert!(!valid(&"0.".repeat(40)));
    }

    #[test]
    fn presets_are_valid_and_render() {
        assert!(!presets().is_empty());
        for (i, spec) in presets().iter().enumerate() {
            assert!(valid(spec), "preset {i}: {spec}");
            parses(&svg(spec, &format!("a{i}")).unwrap());
        }
    }

    #[test]
    fn every_option_renders_with_known_colours() {
        let p = parts();
        let colors = Colors {
            skin: &p.palettes["skin"][0],
            hair: &p.palettes["hairColor"][0],
            bg: &p.palettes["bg"][0],
        };
        let all = p.base.iter().chain(p.shapes.values().flatten().flatten());
        for s in all {
            assert!(
                matches!(s.el.as_str(), "path" | "circle" | "ellipse" | "rect"),
                "unknown element {}",
                s.el
            );
            for c in [&s.fill, &s.stroke].into_iter().flatten() {
                assert!(colors.resolve(c).is_some(), "unknown colour {c}");
            }
        }
        for (i, f) in p.fields.iter().enumerate() {
            for o in 0..p.count(f) {
                let spec = spec_with(i, &o.to_string());
                parses(&svg(&spec, "x").unwrap());
            }
        }
    }

    #[test]
    fn svg_shape() {
        let spec = &presets()[0];
        let out = svg(spec, "av1").unwrap();
        assert!(out.starts_with(r#"<g><defs><clipPath id="av1">"#), "{out}");
        assert!(out.contains(r#"clip-path="url(#av1)""#));
        assert!(out.contains(r#"<rect x="0" y="0" width="100" height="100" fill="#));
        assert!(svg("nope", "x").is_none());
        assert_eq!(num(50.0), "50");
        assert_eq!(num(2.5), "2.5");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(1.0 / 3.0), "0.333");
    }
}
