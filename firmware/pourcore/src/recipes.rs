//! Recipes live in /recipes.json as an array; the web app edits the whole
//! file. Each stage's `water` is the amount added in that stage (grams).
//! Parsing is lenient: a missing or mistyped field takes its default.

use serde_json::Value;

use crate::pattern::{Pattern, PatternParams};

#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub name: String,
    pub water_g: f32,
    pub flow_gps: f32,
    pub pattern: PatternParams,
    pub wait_s: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub min_temp_c: f32,
    pub stages: Vec<Stage>,
}

impl Recipe {
    pub fn total_water(&self) -> f32 {
        self.stages.iter().map(|s| s.water_g).sum()
    }
}

fn num(v: &Value, key: &str, default: f32) -> f32 {
    v.get(key).and_then(Value::as_f64).map_or(default, |n| n as f32)
}

fn text<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn parse_stage(s: &Value) -> Stage {
    Stage {
        name: text(s, "name", "Pour").to_owned(),
        water_g: num(s, "water", 0.0),
        flow_gps: num(s, "flow", 4.0),
        pattern: PatternParams {
            kind: Pattern::parse(text(s, "pattern", "center")),
            radius_mm: num(s, "radius", 20.0),
            rps: num(s, "rps", 1.0),
        },
        wait_s: num(s, "wait", 0.0),
    }
}

/// Finds recipe `id` in the parsed file. Stages that neither pour nor wait are
/// dropped; a recipe left with no stages counts as not found.
pub fn find(file: &Value, id: &str) -> Option<Recipe> {
    let r = file.as_array()?.iter().find(|r| text(r, "id", "") == id)?;
    let stages: Vec<Stage> = r
        .get("stages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(parse_stage)
        .filter(|s| s.water_g > 0.0 || s.wait_s > 0.0)
        .collect();
    if stages.is_empty() {
        return None;
    }
    Some(Recipe {
        id: id.to_owned(),
        name: text(r, "name", "Untitled").to_owned(),
        min_temp_c: num(r, "minTemp", 0.0),
        stages,
    })
}

pub fn first_id(file: &Value) -> Option<String> {
    let id = file.get(0)?.get("id")?.as_str()?;
    Some(id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_with_defaults() {
        let file = json!([
            { "id": "a", "name": "A", "minTemp": 90, "stages": [
                { "name": "Bloom", "water": 40, "pattern": "spiral", "radius": 22, "wait": 30 },
                { "water": 0 },
                { "water": "lots", "wait": 5 }
            ]},
            { "id": "b", "stages": [] }
        ]);
        let r = find(&file, "a").unwrap();
        assert_eq!(r.name, "A");
        assert_eq!(r.min_temp_c, 90.0);
        assert_eq!(r.stages.len(), 2);
        assert_eq!(r.stages[0].pattern.kind, Pattern::Spiral);
        assert_eq!(r.stages[0].flow_gps, 4.0);
        assert_eq!(r.stages[1].name, "Pour");
        assert_eq!(r.total_water(), 40.0);
        assert!(find(&file, "b").is_none());
        assert!(find(&file, "missing").is_none());
        assert_eq!(first_id(&file).as_deref(), Some("a"));
    }

    #[test]
    fn default_recipes_parse() {
        let file: Value =
            serde_json::from_str(include_str!("../../../web/default-recipes.json")).unwrap();
        let id = first_id(&file).unwrap();
        assert!(find(&file, &id).unwrap().total_water() > 0.0);
    }
}
