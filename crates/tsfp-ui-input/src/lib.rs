// SPDX-License-Identifier: GPL-3.0-or-later
//! T2188 UI-5. `UiInput` is the only path from devices to the UI, `UiBindings` is the only path
//! from the UI to the game. See docs/t-ui-input.md. No widget-tree dependency: layouts are seen
//! through the small [`HitTarget`] and [`UiLayout`] traits.

pub mod arcade_category;
pub mod arcade_menu_session;
pub mod bindings;
pub mod challenge_menu;
pub mod default_style_live;
pub mod default_widgets_live;
pub mod editor_pointer;
pub mod frontend_handlers;
pub mod ingame_handlers;
pub mod ingame_live;
pub mod input;
pub mod keyboard;
pub mod layout;
pub mod list_rows_live;
pub mod live_menu;
pub mod live_page;
pub mod mapmaker_action_picker;
pub mod mapmaker_assault_phase;
pub mod mapmaker_assault_phase_assignment;
pub mod mapmaker_assault_phase_toggle;
pub mod mapmaker_assault_start_point_picker;
pub mod mapmaker_award_flags;
pub mod mapmaker_award_medal;
pub mod mapmaker_award_table;
pub mod mapmaker_awards;
pub mod mapmaker_controls_live;
pub mod mapmaker_description;
pub mod mapmaker_disabled_modes;
pub mod mapmaker_document_close;
pub mod mapmaker_editor;
pub mod mapmaker_editor_settings;
pub mod mapmaker_experience;
pub mod mapmaker_handlers;
pub mod mapmaker_kind_list;
pub mod mapmaker_kind_picker;
pub mod mapmaker_logic_summary;
pub mod mapmaker_modes;
pub mod mapmaker_object_kind;
pub mod mapmaker_palette_properties;
// T3799 (rule 37): split into its own crate; the re-export keeps the path.
pub use tsfp_ui_mapmaker_pointer as mapmaker_pointer;
pub mod mapmaker_sample_map_menu;
pub mod mapmaker_set_lists;
pub mod mapmaker_settings;
pub mod mapmaker_storage;
pub mod mapmaker_tool_menu;
pub mod mapmaker_trigger_selection;
pub mod menu_dispatch;
pub mod menu_state;
pub mod model_layout;
pub mod native_handlers;
pub mod native_page;
pub mod native_script;
pub mod native_text;
pub mod owned_catalogue;
pub mod pad_route;
pub mod page_intents;
pub mod page_map;
pub mod page_stack;
pub mod pointer;
pub mod profile_page;
pub mod profile_ports;
pub use tsfp_ui_row_input::repeat;
pub mod selected_entry;
pub mod text_entry;
pub mod text_page;
pub mod text_route;
pub mod widgets;

pub use bindings::{
    ActionArg, ActionId, ActionMap, ActivationOutcome, ActivationSuppression, BindResult, ListId,
    ListItemView, ListView, RecordedCall, RecordingBindings, TextId, UiBindings, UiValue, ValueId,
};
pub use input::{
    ContextId, IntentSet, MenuIntent, Ownership, PageEdgeGate, PlayerFrame, PlayerId, Routed,
    TextEntrySpec, UiEvent, UiInput,
};
pub use layout::{Dir, HitTarget, Rect, UiLayout, WidgetId, hit_test};
pub use live_menu::{
    Dispatch, Goal, LiveMenu, LiveMenuEventDispatch, LiveSlot, PadButton, PadEdge,
    PadRouteBindings, PadRouteDriver, TapTiming,
};
pub use menu_dispatch::dispatch_menu_event;
pub use model_layout::ModelLayout;
pub use page_map::{CursorRow, FrontendPages, PageMap};
pub use pointer::{
    DESIGN_HEIGHT, DESIGN_WIDTH, DesignMapping, PointerButtons, PointerFrame, PointerPolicy,
    UiPointerState,
};
pub use repeat::RepeatPolicy;
pub use text_page::{TextAction, TextPageHandlers, begin_name_entry, dispatch_text_event};
