use std::{collections::HashMap, fmt::Write, sync::OnceLock};

use serde::Deserialize;

const SPEC_MAX: usize = 48;

#[derive(Deserialize)]
struct Def {
    fields: Vec<String>,
    #[serde(default)]
    base: Vec<Shape>,
    #[serde(default)]
    presets: Vec<String>,
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

struct Parts {
    fields: Vec<String>,
    base: Vec<Shape>,
    presets: Vec<String>,
    palettes: HashMap<String, Vec<Vec<String>>>,
    shapes: HashMap<String, Vec<Vec<Shape>>>,
}

const PALETTES: [&str; 3] = ["skin", "hairColor", "bg"];

#[allow(clippy::expect_used)]
fn parts() -> &'static Parts {
    static PARTS: OnceLock<Parts> = OnceLock::new();
    PARTS.get_or_init(|| {
        load_parts(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/avatar.json"
        )))
        .expect("assets/avatar.json is valid")
    })
}

fn load_parts(json: &str) -> Result<Parts, String> {
    let def: Def = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut palettes = HashMap::new();
    let mut shapes = HashMap::new();
    for field in &def.fields {
        let Some(value) = def.options.get(field) else {
            continue;
        };
        if PALETTES.contains(&field.as_str()) {
            let p: Vec<Vec<String>> = serde_json::from_value(value.clone())
                .map_err(|e| format!("palette {field}: {e}"))?;
            palettes.insert(field.clone(), p);
        } else {
            let s: Vec<Vec<Shape>> =
                serde_json::from_value(value.clone()).map_err(|e| format!("part {field}: {e}"))?;
            shapes.insert(field.clone(), s);
        }
    }
    Ok(Parts {
        fields: def.fields,
        base: def.base,
        presets: def.presets,
        palettes,
        shapes,
    })
}

impl Parts {
    fn count(&self, field: &str) -> usize {
        self.palettes
            .get(field)
            .map(Vec::len)
            .or_else(|| self.shapes.get(field).map(Vec::len))
            .unwrap_or(0)
    }

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

pub(crate) fn valid(spec: &str) -> bool {
    parts().parse(spec).is_some()
}

pub fn presets() -> &'static [String] {
    &parts().presets
}

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

struct Colors<'a> {
    skin: &'a [String],
    hair: &'a [String],
    bg: &'a [String],
}

impl Colors<'_> {
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

pub(crate) fn svg(spec: &str, id: &str) -> Option<String> {
    let p = parts();
    let sel = p.parse(spec)?;
    let palette = |field: &str| -> &[String] {
        sel.get(field)
            .and_then(|&i| p.palettes.get(field)?.get(i))
            .map_or(&[], Vec::as_slice)
    };
    let colors = Colors {
        skin: palette("skin"),
        hair: palette("hairColor"),
        bg: palette("bg"),
    };
    let option = |field: &str| -> &[Shape] {
        sel.get(field)
            .and_then(|&i| p.shapes.get(field)?.get(i))
            .map_or(&[], Vec::as_slice)
    };
    let id = esc(id);
    let mut out = format!(
        r#"<g><defs><clipPath id="{id}"><circle cx="50" cy="50" r="50"/></clipPath></defs><g clip-path="url(#{id})">"#
    );
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
    for field in ["hair", "extra"] {
        for s in option(field).iter().filter(|s| s.back()) {
            shape(&mut out, s, &colors);
        }
    }
    for s in &p.base {
        shape(&mut out, s, &colors);
    }
    for field in ["face", "beard", "mouth", "eyes", "brows", "glasses"] {
        for s in option(field) {
            shape(&mut out, s, &colors);
        }
    }
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
            .map(|i| {
                if i == field {
                    value.to_owned()
                } else {
                    "0".into()
                }
            })
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
    fn the_app_draws_from_the_same_parts() {
        assert!(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/avatar.json"))
                == include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../app/src/lib/utils/avatar-parts.json"
                )),
            "server/assets/avatar.json and app/src/lib/avatar-parts.json differ"
        );
    }

    #[test]
    fn the_app_draws_the_mascots_from_the_same_parts() {
        assert!(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/mascot.json"))
                == include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../app/src/lib/utils/mascot-parts.json"
                )),
            "server/assets/mascot.json and app/src/lib/mascot-parts.json differ"
        );
    }

    #[test]
    fn validation() {
        let p = parts();
        let zero = spec_with(usize::MAX, "");
        assert!(valid(&zero));
        for bad in ["", ".", "0", "abc", "-1", " 0"] {
            assert!(!valid(bad), "{bad:?}");
        }
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
