//! Game settings tab, generated from `tsfp_settings::SCHEMA` so it always matches the in-game menu.
//! Placeholder settings are editable and saved but labelled with the system they are waiting for.
use iced::widget::{column, pick_list, row, text, text_input, toggler};
use iced::{Element, Length};
use serde_json::{Value, json};
use tsfp_settings::{Group, Kind, SCHEMA, Setting, Settings, Status};

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

fn shown(settings: &Settings, drafts: &[(&'static str, String)], id: &'static str) -> String {
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

fn control<'a, M: Clone + 'a>(
    s: &'static Setting,
    settings: &Settings,
    drafts: &[(&'static str, String)],
    on: impl Fn(Edit) -> M + Clone + 'a,
) -> Element<'a, M> {
    let id = s.id;
    match s.kind {
        Kind::Bool => {
            let on = on.clone();
            toggler(settings.get(id).and_then(Value::as_bool).unwrap_or(false))
                .on_toggle(move |v| on(Edit::Bool(id, v)))
                .into()
        }
        Kind::Enum(options) => {
            let on = on.clone();
            let selected = settings.get(id).and_then(Value::as_str).map(String::from);
            pick_list(
                options.iter().map(|o| o.to_string()).collect::<Vec<_>>(),
                selected,
                move |v| on(Edit::Choice(id, v)),
            )
            .into()
        }
        Kind::Range { .. } | Kind::List => {
            let on = on.clone();
            text_input(s.default, &shown(settings, drafts, id))
                .on_input(move |v| on(Edit::Text(id, v)))
                .width(Length::Fixed(220.0))
                .into()
        }
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

pub fn view<'a, M: Clone + 'a>(
    settings: &Settings,
    drafts: &[(&'static str, String)],
    on: impl Fn(Edit) -> M + Clone + 'a,
) -> Element<'a, M> {
    let mut col = column![].spacing(6);
    let mut last = None;
    for s in SCHEMA {
        if last != Some(s.group) {
            col = col.push(text(group_name(s.group)).size(20));
            last = Some(s.group);
        }
        let note = match s.status {
            Status::Active => String::new(),
            Status::Placeholder(needs) => {
                format!("  (takes effect once the {needs} system is native)")
            }
        };
        col = col.push(
            row![
                text(s.label.replace('_', " ")).width(Length::Fixed(240.0)),
                control(s, settings, drafts, on.clone()),
                text(note).size(12),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        );
    }
    col.into()
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
