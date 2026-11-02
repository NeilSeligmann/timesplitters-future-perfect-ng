//! Game settings model, generated from `tsfp_settings::SCHEMA` so the launcher always matches the in-game menu.
//! Placeholder settings are editable and saved but labelled with the system they are waiting for.
use serde_json::{Value, json};
use tsfp_settings::{Group, Kind, SCHEMA, Settings, Status};

#[derive(Clone, Debug)]
pub enum Edit {
    Bool(&'static str, bool),
    Choice(&'static str, String),
    Text(&'static str, String),
}

/// Text fields hold what the user typed until it parses; only valid values reach `Settings`.
pub fn apply(settings: &mut Settings, drafts: &mut Vec<(&'static str, String)>, edit: Edit) {
    match edit {
        Edit::Bool(id, v) => {
            settings.set(id, json!(v));
        }
        Edit::Choice(id, v) => {
            settings.set(id, json!(v));
        }
        Edit::Text(id, raw) => {
            let numeric = SCHEMA
                .iter()
                .any(|s| s.id == id && matches!(s.kind, Kind::Range { .. }));
            let value = if numeric {
                raw.trim().parse::<f64>().ok().map(|n| json!(n))
            } else {
                Some(json!(raw))
            };
            if let Some(v) = value {
                settings.set(id, v);
            }
            drafts.retain(|(d, _)| *d != id);
            drafts.push((id, raw));
        }
    }
}

pub fn shown(settings: &Settings, drafts: &[(&'static str, String)], id: &'static str) -> String {
    if let Some((_, raw)) = drafts.iter().find(|(d, _)| *d == id) {
        return raw.clone();
    }
    match settings.get(id) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.as_f64().map_or_else(|| n.to_string(), |f| format!("{f}")),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

pub fn group_name(g: Group) -> &'static str {
    match g {
        Group::Display => "Display",
        Group::Graphics => "Graphics",
        Group::Audio => "Audio",
        Group::Gameplay => "Gameplay",
        Group::Diagnostics => "Diagnostics",
    }
}

/// One row of the settings page. `header` rows start a group and carry only `label`.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub header: bool,
    pub id: &'static str,
    pub label: String,
    /// `bool`, `enum` or `text`.
    pub kind: &'static str,
    pub checked: bool,
    pub text: String,
    pub options: Vec<String>,
    pub selected: i32,
    pub note: String,
}

pub fn rows(settings: &Settings, drafts: &[(&'static str, String)]) -> Vec<Row> {
    let mut out = Vec::new();
    let mut last = None;
    for s in SCHEMA {
        if last != Some(s.group) {
            out.push(Row {
                header: true,
                id: "",
                label: group_name(s.group).into(),
                kind: "",
                checked: false,
                text: String::new(),
                options: Vec::new(),
                selected: -1,
                note: String::new(),
            });
            last = Some(s.group);
        }
        let (kind, options) = match s.kind {
            Kind::Bool => ("bool", Vec::new()),
            Kind::Enum(o) => ("enum", o.iter().map(|x| x.to_string()).collect()),
            Kind::Range { .. } | Kind::List => ("text", Vec::new()),
        };
        let current = settings
            .get(s.id)
            .and_then(Value::as_str)
            .unwrap_or_default();
        out.push(Row {
            header: false,
            id: s.id,
            label: capitalise(&s.label.replace('_', " ")),
            kind,
            checked: settings.get(s.id).and_then(Value::as_bool).unwrap_or(false),
            text: shown(settings, drafts, s.id),
            selected: options
                .iter()
                .position(|o| o == current)
                .map_or(-1, |i| i as i32),
            options,
            note: match s.status {
                Status::Active => String::new(),
                Status::Placeholder(needs) => {
                    format!("Takes effect once the {needs} system is native")
                }
            },
        });
    }
    out
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_text_applies_only_when_valid_and_keeps_draft() {
        let mut s = Settings::default();
        let mut drafts = Vec::new();
        let before = s.get("resolution_width").cloned();
        apply(
            &mut s,
            &mut drafts,
            Edit::Text("resolution_width", "12a".into()),
        );
        assert_eq!(s.get("resolution_width").cloned(), before);
        assert_eq!(shown(&s, &drafts, "resolution_width"), "12a");
        apply(
            &mut s,
            &mut drafts,
            Edit::Text("resolution_width", "2560".into()),
        );
        assert_eq!(
            s.get("resolution_width").and_then(Value::as_f64),
            Some(2560.0)
        );
    }

    #[test]
    fn rows_group_every_schema_setting_with_its_current_value() {
        let mut s = Settings::default();
        let mut drafts = Vec::new();
        apply(
            &mut s,
            &mut drafts,
            Edit::Choice("window_mode", "fullscreen".into()),
        );
        let rows = rows(&s, &drafts);
        assert_eq!(rows.iter().filter(|r| !r.header).count(), SCHEMA.len());
        assert!(rows[0].header && rows[0].label == "Display");
        let wm = rows.iter().find(|r| r.id == "window_mode").unwrap();
        assert_eq!(
            (wm.kind, wm.options[wm.selected as usize].as_str()),
            ("enum", "fullscreen")
        );
        assert_eq!(wm.label, "Window mode");
    }

    #[test]
    fn enum_rejects_values_outside_schema() {
        let mut s = Settings::default();
        let mut drafts = Vec::new();
        apply(
            &mut s,
            &mut drafts,
            Edit::Choice("window_mode", "bogus".into()),
        );
        assert_eq!(
            s.get("window_mode").and_then(Value::as_str),
            Some("windowed")
        );
        apply(
            &mut s,
            &mut drafts,
            Edit::Choice("window_mode", "fullscreen".into()),
        );
        assert_eq!(
            s.get("window_mode").and_then(Value::as_str),
            Some("fullscreen")
        );
    }
}
