//! Map Maker domain boundary reserved by T1812 and extended by T2408.
//! Readable owned UI state is separate from host services and rendering.

extern crate alloc;

pub mod change_entries;
pub mod content;
pub mod controls_ui;
pub mod description;
pub mod disabled_modes;
pub mod disabled_modes_focus;
pub mod document_close;
pub mod edit_view;
pub mod history;
pub mod storage;

pub mod action_picker;
pub mod assault_option_rows;
pub mod assault_options;
pub mod assault_participant_rows;
pub mod assault_phase_assignment;
pub mod assault_phase_toggle;
pub mod assault_start_point_picker;
pub mod award_flags;
pub mod award_flags_rows;
pub mod award_medal_settings;
pub mod award_table;
pub mod awards;
pub mod chunks;
pub mod close_prompt_lifecycle;
pub mod kind_list;
pub mod kind_picker;
pub mod logic_summary;
pub mod mode_choice_rows;
pub mod modes;
pub mod object_kind_chooser;
pub mod overlays;
pub mod sample_map_menu;
pub mod selected_entry;
pub mod tool_menu;
pub mod trigger_selection;
pub mod ui;
