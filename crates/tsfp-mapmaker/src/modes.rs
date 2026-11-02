// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned Map Maker mode, setup-roster and launch data (T2371).
//!
//! Selector namespaces stay explicit: editor kind, theme, preview row, launch variant and
//! launch game type are different values. This module is not wired into the existing default
//! handler path, so constructing these types cannot change current game behaviour.

use tsfp_engine_fileio::map_settings::{MapSettings, SetupEntry};

pub const LAUNCH_BYTES: usize = 0xa0;
pub const ROSTER_SLOTS: usize = 10;
pub const PLAYER_SLOTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorKind(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreviewRow(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchVariant(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameType(pub u8);

/// Semantic UI category. Its identity does not imply a numeric preview variant or game type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMode {
    Deathmatch,
    Bag,
    Escort,
    Assault,
}

/// The original selector values are supplied from the selected preview table row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewSelection {
    pub row: PreviewRow,
    pub variant: LaunchVariant,
    pub game_type: GameType,
}

impl PreviewSelection {
    /// Resolve the measured T1715 preview-table row into its distinct launch
    /// variant and gameplay type. Row identity is not itself either value.
    pub const fn from_row(row: PreviewRow) -> Option<Self> {
        let (variant, game_type) = match row.0 {
            0 => (0, 4),
            1 => (2, 10),
            2 => (3, 9),
            3 => (4, 9),
            4 => (5, 2),
            5 => (6, 1),
            6 => (7, 9),
            7 => (8, 9),
            8 => (9, 9),
            9 => (10, 9),
            10 => (12, 5),
            11 => (15, 6),
            12 => (16, 3),
            13 => (18, 9),
            14 => (19, 9),
            _ => return None,
        };
        Some(Self {
            row,
            variant: LaunchVariant(variant),
            game_type: GameType(game_type),
        })
    }
}

/// Raw settings shared by mode-specific owned configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeSettings {
    pub editor_kind: EditorKind,
    pub theme: ThemeId,
    pub preview: PreviewSelection,
    pub row_options: u8,
    pub duration: u32,
}

/// Phase kind inside Assault. Escort remains a phase namespace, not a launch game-type number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseKind {
    Assault,
    Escort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseSelection {
    pub kind: AssaultPhaseKind,
    pub node_id: u32,
    /// Retain the original raw selection word; toggling uses wrapping `1 - old`.
    pub selected_word: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssaultPhases {
    pub phases: Vec<AssaultPhaseSelection>,
}

/// Typed node row offered to an Assault rule-list rebuild.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultRuleNode {
    pub node_index: u32,
    /// The original enumeration accepts every nonzero widget word as selected.
    pub selected_widget_word: u32,
    /// Preserve the tested source word; first-list metadata checks nonzero.
    pub metadata_word: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultRuleListKind {
    First,
    Second,
    Other(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultRuleFirstEntry {
    pub node_index: u32,
    pub metadata: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultRuleSecondEntry {
    pub node_index: u32,
    pub paired_value: u32,
}

/// Rust-owned rule membership assembled from the page's selected node rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultRuleMembership {
    pub kind: AssaultRuleListKind,
    /// Corresponds to the measured `rule + 0x4c == 1` metadata branch.
    pub add_two_to_first_metadata: bool,
    pub first: Vec<AssaultRuleFirstEntry>,
    pub second: Vec<AssaultRuleSecondEntry>,
}

impl AssaultRuleMembership {
    /// Rebuild selected membership with the measured list-specific append rules.
    ///
    /// The original caller clears only the first count before enumerating rows.
    /// First-list metadata is `(node metadata != 0) + 2` in its special mode;
    /// second-list entries append with a zero paired word. Rust vectors provide
    /// owned storage, so there is no guest capacity check or address arithmetic.
    pub fn rebuild(&mut self, nodes: &[AssaultRuleNode]) {
        self.first.clear();
        for node in nodes.iter().filter(|node| node.selected_widget_word != 0) {
            match self.kind {
                AssaultRuleListKind::First => {
                    let metadata = u8::from(node.metadata_word != 0)
                        + if self.add_two_to_first_metadata { 2 } else { 0 };
                    self.first.push(AssaultRuleFirstEntry {
                        node_index: node.node_index,
                        metadata,
                    });
                }
                AssaultRuleListKind::Second => {
                    self.second.push(AssaultRuleSecondEntry {
                        node_index: node.node_index,
                        paired_value: 0,
                    });
                }
                AssaultRuleListKind::Other(_) => {}
            }
        }
    }
}

/// Candidate or current member supplied to the Assault first-list cost policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultRuleCostNode {
    pub node_index: u32,
    /// The original candidate membership bypass checks node byte `+0xb0 & 2`.
    pub kind_flags: u8,
    /// Original node dword `+8`, selecting the per-node cost policy.
    pub cost_selector: u32,
    /// Typed item-record word `+0x20` for selectors 1, 2 and 9.
    pub item_record_cost: Option<u32>,
    /// Typed item-record word `+0x84`, passed to the selector-8 service.
    pub selector_eight_input: Option<u32>,
}

/// Narrow native boundary for T1402's selector-8 mixed count/state query.
pub trait AssaultRuleCostService {
    type Error;

    fn selector_eight_cost(&mut self, item_word: u32) -> Result<u32, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultBudgetBypass {
    NoActiveContext,
    Disabled,
    OtherKind,
    AlreadyInFirstList,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssaultBudgetUnavailable<E> {
    MissingItemCost { node_index: u32, selector: u32 },
    MissingSelectorEightInput { node_index: u32 },
    SelectorEight(E),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssaultBudgetDecision<E> {
    Bypassed(AssaultBudgetBypass),
    WithinBudget { signed_total: i32 },
    Exceeded { signed_total: i32 },
    Unavailable(AssaultBudgetUnavailable<E>),
}

/// Apply T1688's first-rule-list cost policy to Rust-owned typed nodes.
///
/// The descriptor-capacity check is a separate page decision. Here, an absent
/// active context, disabled positive budget, non-one kind, or already-member
/// special node follows the original cost-bypass paths. Selector 8 remains an
/// explicit service call because T1402's result is not a generic item count.
pub fn check_assault_rule_budget<S: AssaultRuleCostService>(
    has_active_context: bool,
    active_budget: i32,
    kind: u32,
    first_rule_nodes: &[AssaultRuleCostNode],
    candidate: &AssaultRuleCostNode,
    service: &mut S,
) -> AssaultBudgetDecision<S::Error> {
    if !has_active_context {
        return AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::NoActiveContext);
    }
    if active_budget <= 0 {
        return AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::Disabled);
    }
    if kind != 1 {
        return AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::OtherKind);
    }
    if candidate.kind_flags & 2 != 0
        && first_rule_nodes
            .iter()
            .any(|node| node.node_index == candidate.node_index)
    {
        return AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::AlreadyInFirstList);
    }

    let mut total = 0u32;
    for node in first_rule_nodes.iter().chain([candidate]) {
        match assault_rule_node_cost(node, service) {
            Ok(cost) => total = total.wrapping_add(cost),
            Err(error) => return AssaultBudgetDecision::Unavailable(error),
        }
    }

    let signed_total = total as i32;
    if signed_total <= 10 {
        AssaultBudgetDecision::WithinBudget { signed_total }
    } else {
        AssaultBudgetDecision::Exceeded { signed_total }
    }
}

fn assault_rule_node_cost<S: AssaultRuleCostService>(
    node: &AssaultRuleCostNode,
    service: &mut S,
) -> Result<u32, AssaultBudgetUnavailable<S::Error>> {
    match node.cost_selector {
        0 | 3 | 4 | 7 => Ok(1),
        1 | 2 | 9 => node
            .item_record_cost
            .ok_or(AssaultBudgetUnavailable::MissingItemCost {
                node_index: node.node_index,
                selector: node.cost_selector,
            }),
        8 => {
            let input = node.selector_eight_input.ok_or(
                AssaultBudgetUnavailable::MissingSelectorEightInput {
                    node_index: node.node_index,
                },
            )?;
            service
                .selector_eight_cost(input)
                .map_err(AssaultBudgetUnavailable::SelectorEight)
        }
        _ => Ok(0),
    }
}

/// Rust-owned decision from the Assault phase selection availability check.
///
/// This reports the distinct measured capacity and weighted-cost refusals
/// without assigning either one to a particular popup or renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssaultPhaseAvailability<E> {
    NotSelected,
    CapacityRefused,
    BudgetRefused { signed_total: i32 },
    Available,
    Unavailable(AssaultBudgetUnavailable<E>),
}

/// Localized popup identity supplied by the active phase descriptor.
///
/// The original page reads the label ID and numeric argument from separate
/// descriptor fields. Keeping both values explicit avoids baking a partial
/// descriptor table into the Rust-owned UI model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhasePopupRequest {
    pub label_id: u32,
    pub argument: u32,
}

pub const ASSAULT_PHASE_BUDGET_LABEL_ID: u32 = 0x0bba;
pub const ASSAULT_PHASE_BUDGET_LIMIT: u32 = 10;

/// Post-check effects for the Assault phase page.
///
/// This describes the original result-specific ordering after the caller has
/// already performed the phase-list refresh and availability check: ordinary
/// results rebuild rows before refreshing the widget; refusals show their
/// popup before the final widget refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseUiPlan {
    RefreshRowsThenWidget,
    PopupThenWidget(AssaultPhasePopupRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssaultPhaseUiPlanError<E> {
    MissingCapacityMessage,
    BudgetUnavailable(AssaultBudgetUnavailable<E>),
}

/// Map a typed availability result to the result-specific popup/refresh plan.
///
/// Capacity text is supplied by the active descriptor (`+0x30` label ID and
/// `+0x2c` argument); the static REA evidence only samples part of that table.
/// The weighted-budget refusal uses the measured fixed label `0x0bba` and
/// argument ten. Unavailable cost data stays an error instead of becoming a
/// fabricated success or popup.
pub fn plan_assault_phase_ui<E>(
    availability: AssaultPhaseAvailability<E>,
    capacity_message: Option<AssaultPhasePopupRequest>,
) -> Result<AssaultPhaseUiPlan, AssaultPhaseUiPlanError<E>> {
    match availability {
        AssaultPhaseAvailability::NotSelected | AssaultPhaseAvailability::Available => {
            Ok(AssaultPhaseUiPlan::RefreshRowsThenWidget)
        }
        AssaultPhaseAvailability::CapacityRefused => capacity_message
            .map(AssaultPhaseUiPlan::PopupThenWidget)
            .ok_or(AssaultPhaseUiPlanError::MissingCapacityMessage),
        AssaultPhaseAvailability::BudgetRefused { .. } => Ok(AssaultPhaseUiPlan::PopupThenWidget(
            AssaultPhasePopupRequest {
                label_id: ASSAULT_PHASE_BUDGET_LABEL_ID,
                argument: ASSAULT_PHASE_BUDGET_LIMIT,
            },
        )),
        AssaultPhaseAvailability::Unavailable(error) => {
            Err(AssaultPhaseUiPlanError::BudgetUnavailable(error))
        }
    }
}

/// Resolve the measured phase-selection gate in its original order.
///
/// The caller passes the post-toggle raw widget word. Only exactly `1` is
/// eligible for either disabling arm; other nonzero words remain distinct
/// and produce the original no-rejection result. Descriptor capacity is
/// checked before weighted cost, and selector-service failures stay explicit.
pub fn resolve_assault_phase_availability<E, F>(
    selected_widget_word: u32,
    descriptor_capacity_reached: bool,
    evaluate_budget: F,
) -> AssaultPhaseAvailability<E>
where
    F: FnOnce() -> AssaultBudgetDecision<E>,
{
    if selected_widget_word != 1 {
        return AssaultPhaseAvailability::NotSelected;
    }
    if descriptor_capacity_reached {
        return AssaultPhaseAvailability::CapacityRefused;
    }

    match evaluate_budget() {
        AssaultBudgetDecision::Exceeded { signed_total } => {
            AssaultPhaseAvailability::BudgetRefused { signed_total }
        }
        AssaultBudgetDecision::Unavailable(error) => AssaultPhaseAvailability::Unavailable(error),
        AssaultBudgetDecision::Bypassed(_) | AssaultBudgetDecision::WithinBudget { .. } => {
            AssaultPhaseAvailability::Available
        }
    }
}

/// Result of a native phase-row toggle against Rust-owned phase state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseToggleResult {
    Changed { before: u32, after: u32 },
    Disabled,
    Missing,
    Ambiguous,
}

/// Typed handoff between the original phase toggle and its post-refresh
/// capacity/cost check. The caller applies its page refresh after receiving
/// this receipt, then resolves availability with the current owned services.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseToggleReceipt {
    pub kind: AssaultPhaseKind,
    pub node_id: u32,
    pub before: u32,
    pub after: u32,
    /// Raw selected-widget value is distinct from the phase's raw data word.
    pub post_toggle_widget_word: u32,
}

impl AssaultPhaseToggleReceipt {
    /// Resolve the measured follow-up gate after the caller refreshes the
    /// visible phase list. Capacity is evaluated before weighted cost, and
    /// service failures remain explicit.
    pub fn resolve_after_refresh<E, F>(
        self,
        descriptor_capacity_reached: bool,
        evaluate_budget: F,
    ) -> AssaultPhaseAvailability<E>
    where
        F: FnOnce() -> AssaultBudgetDecision<E>,
    {
        resolve_assault_phase_availability(
            self.post_toggle_widget_word,
            descriptor_capacity_reached,
            evaluate_budget,
        )
    }
}

impl AssaultPhases {
    /// Apply the measured phase toggle using an explicit semantic identity.
    ///
    /// The original checks both widget selectability and descriptor enablement,
    /// then stores wrapping `1 - old` before its follow-up availability check.
    /// The native caller supplies those two typed gates and never passes guest
    /// addresses or a cursor-derived array offset.
    pub fn toggle(
        &mut self,
        kind: AssaultPhaseKind,
        node_id: u32,
        widget_selectable: bool,
        descriptor_enabled: bool,
    ) -> AssaultPhaseToggleResult {
        if !widget_selectable || !descriptor_enabled {
            return AssaultPhaseToggleResult::Disabled;
        }
        let mut matches = self
            .phases
            .iter()
            .enumerate()
            .filter(|(_, phase)| phase.kind == kind && phase.node_id == node_id);
        let Some((index, _)) = matches.next() else {
            return AssaultPhaseToggleResult::Missing;
        };
        if matches.next().is_some() {
            return AssaultPhaseToggleResult::Ambiguous;
        }
        let phase = &mut self.phases[index];
        let before = phase.selected_word;
        let after = toggle_phase_selection(before);
        phase.selected_word = after;
        AssaultPhaseToggleResult::Changed { before, after }
    }

    /// Toggle one Rust-owned phase and return the typed follow-up check.
    ///
    /// The caller must perform the original page refresh before calling
    /// `AssaultPhaseToggleReceipt::resolve_after_refresh`; this keeps the
    /// measured toggle → refresh → availability order visible in the API.
    pub fn toggle_with_availability_receipt(
        &mut self,
        kind: AssaultPhaseKind,
        node_id: u32,
        widget_selectable: bool,
        descriptor_enabled: bool,
        post_toggle_widget_word: u32,
    ) -> Result<AssaultPhaseToggleReceipt, AssaultPhaseToggleResult> {
        match self.toggle(kind, node_id, widget_selectable, descriptor_enabled) {
            AssaultPhaseToggleResult::Changed { before, after } => Ok(AssaultPhaseToggleReceipt {
                kind,
                node_id,
                before,
                after,
                post_toggle_widget_word,
            }),
            unchanged => Err(unchanged),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultModeConfiguration {
    pub settings: ModeSettings,
    pub phases: AssaultPhases,
}

/// Every selectable Map Maker configuration carries its own selector snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModeConfiguration {
    Deathmatch(ModeSettings),
    Bag(ModeSettings),
    Escort(ModeSettings),
    Assault(AssaultModeConfiguration),
}

impl ModeConfiguration {
    pub fn kind(&self) -> MapMode {
        match self {
            Self::Deathmatch(_) => MapMode::Deathmatch,
            Self::Bag(_) => MapMode::Bag,
            Self::Escort(_) => MapMode::Escort,
            Self::Assault(_) => MapMode::Assault,
        }
    }

    pub fn settings(&self) -> &ModeSettings {
        match self {
            Self::Deathmatch(settings) | Self::Bag(settings) | Self::Escort(settings) => settings,
            Self::Assault(assault) => &assault.settings,
        }
    }
}

pub fn toggle_phase_selection(selected_word: u32) -> u32 {
    1u32.wrapping_sub(selected_word)
}

/// A setup entry after conversion to the launch helper's fixed stride-eight table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetupSlot {
    pub participant_id: i16,
    /// Low byte of the sign-extended source secondary field. It is copied only for selected IDs.
    pub conditional_source: u8,
}

impl Default for SetupSlot {
    fn default() -> Self {
        Self {
            participant_id: 0,
            conditional_source: 0,
        }
    }
}

/// Source and lifetime are explicit because the original editor/profile decoders reuse buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RosterSource {
    BuiltIn {
        preset: i16,
    },
    ContextualChunk {
        selector: i16,
        epoch: u64,
    },
    Editor {
        epoch: u64,
    },
    ProfileCustom {
        selector: i16,
        profile_slot: u8,
        epoch: u64,
    },
}

/// Owned ten-slot setup roster; `raw_count` remains byte-sized and padding stays explicit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupRoster {
    pub source: RosterSource,
    pub raw_count: u8,
    pub slots: [SetupSlot; ROSTER_SLOTS],
}

impl SetupRoster {
    /// Copy the owned file model into the original setup-helper shape.
    /// Unused slots are zero-filled, matching the original scratch constructor.
    pub fn from_map_settings(settings: &MapSettings, source: RosterSource) -> Option<Self> {
        if settings.setups.len() > ROSTER_SLOTS {
            return None;
        }
        let mut slots = [SetupSlot::default(); ROSTER_SLOTS];
        for (slot, entry) in slots.iter_mut().zip(settings.setups.iter().copied()) {
            *slot = from_wire_entry(entry);
        }
        Some(Self {
            source,
            raw_count: settings.setups.len() as u8,
            slots,
        })
    }

    pub fn from_wire_entries(source: RosterSource, entries: &[SetupEntry]) -> Option<Self> {
        if entries.len() > ROSTER_SLOTS {
            return None;
        }
        let mut slots = [SetupSlot::default(); ROSTER_SLOTS];
        for (slot, entry) in slots.iter_mut().zip(entries.iter().copied()) {
            *slot = from_wire_entry(entry);
        }
        Some(Self {
            source,
            raw_count: entries.len() as u8,
            slots,
        })
    }
}

fn from_wire_entry(entry: SetupEntry) -> SetupSlot {
    SetupSlot {
        participant_id: entry.option.0,
        conditional_source: entry.secondary as u8,
    }
}

/// Inputs queried by the lifted setup helper. The caller owns the source table and ID services.
pub trait RosterMetadata {
    fn has_conditional_metadata(&mut self, participant_id: i16) -> bool;
    fn derived_metadata(&mut self, participant_id: i16) -> u8;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Participant {
    pub id: u8,
    pub conditional_metadata: u8,
    pub derived_metadata: u8,
}

/// Launch helper output uses slots 1..N; slot zero remains the local player entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParticipantRoster {
    pub emitted_count: u8,
    pub ids: [u8; PLAYER_SLOTS],
    pub conditional: [u8; PLAYER_SLOTS],
    pub derived: [u8; PLAYER_SLOTS],
}

impl ParticipantRoster {
    pub fn empty() -> Self {
        let mut ids = [0; PLAYER_SLOTS];
        ids[0] = 0x0c;
        Self {
            emitted_count: 0,
            ids,
            conditional: [0xff; PLAYER_SLOTS],
            derived: [0; PLAYER_SLOTS],
        }
    }
}

/// Reproduce `0x3125a0`: scan all ten records, skip -1 entries and never stop at a hole.
/// `maximum` is signed because the original compares its nonnegative emitted index as signed.
pub fn emit_setup_roster<M: RosterMetadata>(
    setup: &[SetupSlot; ROSTER_SLOTS],
    maximum: i32,
    metadata: &mut M,
) -> ParticipantRoster {
    let mut roster = ParticipantRoster::empty();
    if maximum <= 0 {
        return roster;
    }
    let limit = (maximum as usize).min(PLAYER_SLOTS - 1);
    let mut emitted = 0usize;
    for entry in setup {
        if entry.participant_id == -1 || emitted >= limit {
            continue;
        }
        let slot = emitted + 1;
        roster.ids[slot] = entry.participant_id as u8;
        if metadata.has_conditional_metadata(entry.participant_id) {
            roster.conditional[slot] = entry.conditional_source;
        }
        roster.derived[slot] = metadata.derived_metadata(entry.participant_id);
        emitted += 1;
    }
    roster.emitted_count = emitted as u8;
    roster
}

/// Numeric classification used only by the special variant-0x13 launch branch.
pub fn is_special_participant_id(id: u8) -> bool {
    matches!(id, 0x5d | 0x5e | 0x5f | 0x65 | 0x9b)
}

/// One native launch configuration, independent of the guest-memory launch block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchConfiguration {
    pub local_slots: u8,
    pub participant_count: u8,
    pub team_policy: [u8; 3],
    pub participants: ParticipantRoster,
    pub teams: [u8; PLAYER_SLOTS],
    pub derived: [u8; PLAYER_SLOTS],
    pub conditional: [u8; PLAYER_SLOTS],
    pub state: u8,
    pub game_type: GameType,
    pub variant: u8,
    pub active: u8,
    pub flags: u32,
    pub duration: u32,
    /// The values written to globals 0x784074 and 0x784078, in draw order.
    pub rng_values: [u32; 2],
    pub policy_64: u32,
    pub policy_74: u32,
    pub policy_8c: u32,
}

impl LaunchConfiguration {
    /// Build the original 0xa0-byte handoff record from owned inputs.
    pub fn build(input: &LaunchInput, participants: Option<&ParticipantRoster>) -> Self {
        let mode = input.mode.settings();
        let variant = mode.preview.variant.0;
        let variant_byte = variant as u8;
        let mut config = Self {
            local_slots: if mode.row_options & 0x02 == 0 {
                input.local_player_count.wrapping_add(1)
            } else {
                input.setup_count as u8
            },
            participant_count: 0,
            team_policy: [0; 3],
            participants: ParticipantRoster::empty(),
            teams: [0; PLAYER_SLOTS],
            derived: [0; PLAYER_SLOTS],
            conditional: [0xff; PLAYER_SLOTS],
            state: 0x64,
            game_type: mode.preview.game_type,
            variant: variant_byte,
            active: 1,
            flags: if variant == 2 {
                0x0408_0008
            } else {
                0x040d_0000
            },
            duration: 0,
            rng_values: [0; 2],
            policy_64: 0,
            policy_74: 0,
            policy_8c: 0,
        };
        config.conditional[0..PLAYER_SLOTS].fill(0xff);
        config.participants.ids[0] = 0x0c;
        // The helper's emitted count is a local loop bound, not launch byte +0x02. The
        // original leaves that record byte zero on ordinary variants.
        let mut emitted_count = 0usize;
        if variant != 2 && variant_byte != 0 {
            config.flags |= 0x8402;
        }

        // The original compares full variant values for roster admission and Story policy.
        if input.launch_argument_missing && variant != 0 && variant != 2 {
            if let Some(roster) = participants {
                // Setup helper output is already bounded by the setup source's signed
                // count byte. `input.setup_count` is the separate launch global used for
                // local-slot policy and must not truncate that roster a second time.
                emitted_count = usize::from(roster.emitted_count).min(PLAYER_SLOTS - 1);
                config.participants = roster.clone();
                config.derived = config.participants.derived;
                config.conditional = config.participants.conditional;
                config.participants.emitted_count = emitted_count as u8;
                // 0x3125a0 returns the count that 0x313a80 stores at record +0x02.
                // Variant 0x11 replaces it and variant 0x13 increments it below.
                config.participant_count = emitted_count as u8;
            }
        }
        if variant != 2 && mode.row_options & 0x01 != 0 {
            config.duration = mode.duration;
        }

        for index in 0..emitted_count {
            let slot = match variant_byte {
                4 | 5 | 0x0f => index + 1,
                0x10 | 0x11 => usize::from(config.local_slots).wrapping_add(index),
                _ => index + 1,
            };
            if slot < PLAYER_SLOTS {
                config.teams[slot] = match variant_byte {
                    4 | 5 | 0x0f => {
                        if index % 2 == 0 {
                            2
                        } else {
                            1
                        }
                    }
                    0x10 => {
                        if index % 2 == 0 {
                            1
                        } else {
                            2
                        }
                    }
                    0x11 => {
                        if index % 2 == 0 {
                            2
                        } else {
                            1
                        }
                    }
                    _ => 0,
                };
            }
        }

        match variant_byte {
            4 | 5 | 0x0f => {
                config.flags |= 1;
                config.team_policy = [2, 1, 2];
                config.teams[0] = 1;
            }
            7 => {
                config.policy_74 = 10;
                config.policy_64 = 0;
            }
            9 => {
                config.flags = (config.flags & !0x10) | 0x20;
                config.policy_8c = 1.0f32.to_bits();
                config.policy_74 = 10;
                config.policy_64 = 0;
            }
            0x0c => {
                config.game_type = GameType(5);
                config.variant = 0x0c;
            }
            0x10 => {
                config.flags |= 1;
                config.team_policy = [2, 1, 2];
                for index in 0..2 {
                    let roll = input.random_draws[index] % 6;
                    let slot = usize::from(config.local_slots).wrapping_add(index);
                    if slot < PLAYER_SLOTS {
                        config.teams[slot] = input.team_values[slot].wrapping_add(1);
                    }
                    config.rng_values[index] = roll;
                }
            }
            0x11 => {
                config.flags |= 0x401;
                config.participant_count = 8u8.wrapping_sub(config.local_slots);
                config.team_policy = [2, 2, 1];
                config.teams[0] = 1;
            }
            0x13 => {
                let classified = (0..emitted_count)
                    .filter(|index| {
                        let slot = usize::from(config.local_slots).wrapping_add(*index);
                        slot < PLAYER_SLOTS
                            && is_special_participant_id(config.participants.ids[slot])
                    })
                    .count();
                let missing = 3usize.saturating_sub(classified);
                for index in 0..missing {
                    let slot = usize::from(config.local_slots)
                        .wrapping_add(usize::from(config.participant_count));
                    if slot < PLAYER_SLOTS {
                        config.participants.ids[slot] = 0x5d;
                        config.participants.conditional[slot] = 0xff;
                        config.conditional[slot] = 0xff;
                        config.participant_count = config.participant_count.wrapping_add(1);
                    }
                    let _ = index;
                }
                config.participants.emitted_count = config.participant_count;
            }
            _ => {}
        }

        if matches!(variant_byte, 0x10 | 0x11) {
            let clear_from = usize::from(config.team_policy[0]).saturating_mul(4);
            if clear_from < PLAYER_SLOTS {
                config.teams[clear_from..].fill(0);
            }
        }
        config
    }

    /// Pure compatibility encoding used by differential tests to compare the measured layout.
    /// Native launch callers pass `Self` to `LaunchCoordinator`; they do not use this encoding.
    pub fn to_original_bytes(&self) -> [u8; LAUNCH_BYTES] {
        let mut bytes = [0; LAUNCH_BYTES];
        bytes[0] = self.local_slots;
        bytes[2] = self.participant_count;
        bytes[3..6].copy_from_slice(&self.team_policy);
        bytes[0x14..0x24].copy_from_slice(&self.participants.ids);
        bytes[0x24..0x34].copy_from_slice(&self.teams);
        bytes[0x34..0x44].copy_from_slice(&self.derived);
        bytes[0x44..0x54].copy_from_slice(&self.conditional);
        bytes[0x54] = self.state;
        bytes[0x55] = self.game_type.0;
        bytes[0x56] = self.variant;
        bytes[0x57] = self.active;
        bytes[0x58..0x5c].copy_from_slice(&self.flags.to_le_bytes());
        bytes[0x60..0x64].copy_from_slice(&self.duration.to_le_bytes());
        bytes[0x64..0x68].copy_from_slice(&self.policy_64.to_le_bytes());
        bytes[0x74..0x78].copy_from_slice(&self.policy_74.to_le_bytes());
        bytes[0x8c..0x90].copy_from_slice(&self.policy_8c.to_le_bytes());
        bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchInput {
    pub mode: ModeConfiguration,
    pub local_player_count: u8,
    pub setup_count: i8,
    pub launch_argument_missing: bool,
    pub team_values: [u8; PLAYER_SLOTS],
    /// Two values returned by the injected RNG for variant 0x10; order is significant.
    pub random_draws: [u32; 2],
}

/// Native launch boundary. Implementations receive owned typed configuration directly.
pub trait LaunchCoordinator {
    fn launch(&mut self, configuration: LaunchConfiguration);
}

pub fn handoff<C: LaunchCoordinator>(
    coordinator: &mut C,
    input: &LaunchInput,
    participants: Option<&ParticipantRoster>,
) {
    coordinator.launch(LaunchConfiguration::build(input, participants));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_roster_copies_signed_wire_fields_and_zero_fills_padding() {
        let mut bytes = vec![0; tsfp_engine_fileio::map_settings::SETTINGS_BYTES];
        bytes[0x16] = 2;
        bytes[0x18..0x1a].copy_from_slice(&(-1i16).to_le_bytes());
        bytes[0x1b] = 0x80;
        bytes[0x1c..0x1e].copy_from_slice(&(36i16).to_le_bytes());
        bytes[0x1f] = 0x7f;
        let settings = MapSettings::decode(&bytes).unwrap();
        let roster =
            SetupRoster::from_map_settings(&settings, RosterSource::Editor { epoch: 3 }).unwrap();
        assert_eq!(roster.raw_count, 2);
        assert_eq!(roster.slots[0].participant_id, -1);
        assert_eq!(roster.slots[0].conditional_source, 0x80);
        assert_eq!(roster.slots[1].participant_id, 36);
        assert_eq!(roster.slots[1].conditional_source, 0x7f);
        assert_eq!(roster.slots[2], SetupSlot::default());
    }

    #[test]
    fn mode_variant_and_game_type_are_separate_typed_values() {
        let settings = ModeSettings {
            editor_kind: EditorKind(1),
            theme: ThemeId(3),
            preview: PreviewSelection {
                row: PreviewRow(12),
                variant: LaunchVariant(16),
                game_type: GameType(3),
            },
            row_options: 0,
            duration: 0,
        };
        let assault = ModeConfiguration::Assault(AssaultModeConfiguration {
            settings,
            phases: AssaultPhases {
                phases: vec![AssaultPhaseSelection {
                    kind: AssaultPhaseKind::Escort,
                    node_id: 4,
                    selected_word: 2,
                }],
            },
        });
        assert_eq!(assault.kind(), MapMode::Assault);
        assert_eq!(assault.settings().preview.variant, LaunchVariant(16));
        assert_eq!(assault.settings().preview.game_type, GameType(3));
        assert_eq!(toggle_phase_selection(2), u32::MAX);
    }

    #[test]
    fn phase_toggle_receipt_preserves_refresh_then_capacity_and_budget_order() {
        let mut phases = AssaultPhases {
            phases: vec![AssaultPhaseSelection {
                kind: AssaultPhaseKind::Assault,
                node_id: 12,
                selected_word: 2,
            }],
        };
        let receipt = phases
            .toggle_with_availability_receipt(AssaultPhaseKind::Assault, 12, true, true, 1)
            .unwrap();

        // The source toggles the raw phase word before its page refresh and
        // only then performs the follow-up availability check.
        assert_eq!(receipt.before, 2);
        assert_eq!(receipt.after, u32::MAX);
        assert_eq!(phases.phases[0].selected_word, u32::MAX);
        let mut evaluated_budget = false;
        assert_eq!(
            receipt.resolve_after_refresh(true, || {
                evaluated_budget = true;
                AssaultBudgetDecision::<&str>::Exceeded { signed_total: 11 }
            }),
            AssaultPhaseAvailability::CapacityRefused
        );
        assert!(!evaluated_budget);

        let receipt = phases
            .toggle_with_availability_receipt(AssaultPhaseKind::Assault, 12, true, true, 1)
            .unwrap();
        assert_eq!(receipt.after, 2);
        assert_eq!(
            receipt.resolve_after_refresh(false, || {
                AssaultBudgetDecision::<&str>::Exceeded { signed_total: 11 }
            }),
            AssaultPhaseAvailability::BudgetRefused { signed_total: 11 }
        );
    }

    #[test]
    fn rejected_phase_toggle_produces_no_availability_receipt() {
        let mut phases = AssaultPhases {
            phases: vec![AssaultPhaseSelection {
                kind: AssaultPhaseKind::Assault,
                node_id: 12,
                selected_word: 1,
            }],
        };
        assert_eq!(
            phases.toggle_with_availability_receipt(AssaultPhaseKind::Assault, 12, false, true, 1,),
            Err(AssaultPhaseToggleResult::Disabled)
        );
        assert_eq!(phases.phases[0].selected_word, 1);
    }

    #[test]
    fn assault_rule_first_list_uses_nonzero_words_and_preserves_second_list() {
        let mut membership = AssaultRuleMembership {
            kind: AssaultRuleListKind::First,
            add_two_to_first_metadata: true,
            first: vec![AssaultRuleFirstEntry {
                node_index: 99,
                metadata: 9,
            }],
            second: vec![AssaultRuleSecondEntry {
                node_index: 88,
                paired_value: 7,
            }],
        };

        membership.rebuild(&[
            AssaultRuleNode {
                node_index: 1,
                selected_widget_word: 0,
                metadata_word: u32::MAX,
            },
            AssaultRuleNode {
                node_index: 2,
                selected_widget_word: 2,
                metadata_word: 0,
            },
            AssaultRuleNode {
                node_index: 3,
                selected_widget_word: u32::MAX,
                metadata_word: 4,
            },
        ]);

        assert_eq!(
            membership.first,
            vec![
                AssaultRuleFirstEntry {
                    node_index: 2,
                    metadata: 2,
                },
                AssaultRuleFirstEntry {
                    node_index: 3,
                    metadata: 3,
                },
            ]
        );
        assert_eq!(
            membership.second,
            vec![AssaultRuleSecondEntry {
                node_index: 88,
                paired_value: 7,
            }]
        );
    }

    #[test]
    fn assault_rule_second_list_appends_zero_pairs_and_other_kind_does_nothing() {
        let mut membership = AssaultRuleMembership {
            kind: AssaultRuleListKind::Second,
            add_two_to_first_metadata: false,
            first: vec![AssaultRuleFirstEntry {
                node_index: 99,
                metadata: 1,
            }],
            second: vec![AssaultRuleSecondEntry {
                node_index: 88,
                paired_value: 7,
            }],
        };
        membership.rebuild(&[
            AssaultRuleNode {
                node_index: 4,
                selected_widget_word: 1,
                metadata_word: 0,
            },
            AssaultRuleNode {
                node_index: 5,
                selected_widget_word: 0,
                metadata_word: 1,
            },
        ]);
        assert!(membership.first.is_empty());
        assert_eq!(
            membership.second,
            vec![
                AssaultRuleSecondEntry {
                    node_index: 88,
                    paired_value: 7,
                },
                AssaultRuleSecondEntry {
                    node_index: 4,
                    paired_value: 0,
                },
            ]
        );

        membership.kind = AssaultRuleListKind::Other(3);
        membership.first.push(AssaultRuleFirstEntry {
            node_index: 77,
            metadata: 2,
        });
        let second_before = membership.second.clone();
        membership.rebuild(&[AssaultRuleNode {
            node_index: 6,
            selected_widget_word: 1,
            metadata_word: 1,
        }]);
        assert!(membership.first.is_empty());
        assert_eq!(membership.second, second_before);
    }

    struct TestAssaultCostService {
        inputs: Vec<u32>,
        answer: Result<u32, &'static str>,
    }

    impl AssaultRuleCostService for TestAssaultCostService {
        type Error = &'static str;

        fn selector_eight_cost(&mut self, item_word: u32) -> Result<u32, Self::Error> {
            self.inputs.push(item_word);
            self.answer
        }
    }

    fn cost_node(
        node_index: u32,
        kind_flags: u8,
        cost_selector: u32,
        item_record_cost: Option<u32>,
        selector_eight_input: Option<u32>,
    ) -> AssaultRuleCostNode {
        AssaultRuleCostNode {
            node_index,
            kind_flags,
            cost_selector,
            item_record_cost,
            selector_eight_input,
        }
    }

    #[test]
    fn assault_rule_budget_sums_measured_selectors_and_calls_selector_eight_service() {
        let existing = [
            cost_node(1, 0, 0, None, None),
            cost_node(2, 0, 2, Some(3), None),
        ];
        let candidate = cost_node(3, 0, 8, None, Some(0x55));
        let mut service = TestAssaultCostService {
            inputs: Vec::new(),
            answer: Ok(4),
        };

        assert_eq!(
            check_assault_rule_budget(true, 1, 1, &existing, &candidate, &mut service),
            AssaultBudgetDecision::WithinBudget { signed_total: 8 }
        );
        assert_eq!(service.inputs, vec![0x55]);
    }

    #[test]
    fn assault_rule_budget_covers_each_measured_selector_group() {
        let mut service = TestAssaultCostService {
            inputs: Vec::new(),
            answer: Ok(4),
        };

        for selector in [0, 3, 4, 7] {
            let candidate = cost_node(1, 0, selector, None, None);
            assert_eq!(
                check_assault_rule_budget(true, 1, 1, &[], &candidate, &mut service),
                AssaultBudgetDecision::WithinBudget { signed_total: 1 },
                "selector {selector}"
            );
        }
        for selector in [1, 2, 9] {
            let candidate = cost_node(2, 0, selector, Some(2), None);
            assert_eq!(
                check_assault_rule_budget(true, 1, 1, &[], &candidate, &mut service),
                AssaultBudgetDecision::WithinBudget { signed_total: 2 },
                "selector {selector}"
            );
        }
        let selector_eight = cost_node(3, 0, 8, None, Some(0x84));
        assert_eq!(
            check_assault_rule_budget(true, 1, 1, &[], &selector_eight, &mut service),
            AssaultBudgetDecision::WithinBudget { signed_total: 4 }
        );
        for selector in [5, 6, 10, u32::MAX] {
            let candidate = cost_node(4, 0, selector, None, None);
            assert_eq!(
                check_assault_rule_budget(true, 1, 1, &[], &candidate, &mut service),
                AssaultBudgetDecision::WithinBudget { signed_total: 0 },
                "selector {selector}"
            );
        }
        assert_eq!(service.inputs, vec![0x84]);
    }

    #[test]
    fn assault_rule_budget_returns_distinct_bypasses_before_cost_evaluation() {
        let candidate = cost_node(3, 2, 8, None, None);
        let present = [candidate];
        let mut service = TestAssaultCostService {
            inputs: Vec::new(),
            answer: Ok(0),
        };

        assert_eq!(
            check_assault_rule_budget(false, 1, 1, &[], &candidate, &mut service),
            AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::NoActiveContext)
        );
        assert_eq!(
            check_assault_rule_budget(true, 0, 1, &[], &candidate, &mut service),
            AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::Disabled)
        );
        assert_eq!(
            check_assault_rule_budget(true, -1, 1, &[], &candidate, &mut service),
            AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::Disabled)
        );
        assert_eq!(
            check_assault_rule_budget(true, 1, 2, &[], &candidate, &mut service),
            AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::OtherKind)
        );
        assert_eq!(
            check_assault_rule_budget(true, 1, 1, &present, &candidate, &mut service),
            AssaultBudgetDecision::Bypassed(AssaultBudgetBypass::AlreadyInFirstList)
        );
        assert!(service.inputs.is_empty());
    }

    #[test]
    fn assault_rule_budget_keeps_selector_eight_unavailable_instead_of_zero_cost() {
        let candidate = cost_node(3, 0, 8, None, None);
        let mut service = TestAssaultCostService {
            inputs: Vec::new(),
            answer: Ok(0),
        };
        assert_eq!(
            check_assault_rule_budget(true, 1, 1, &[], &candidate, &mut service),
            AssaultBudgetDecision::Unavailable(
                AssaultBudgetUnavailable::MissingSelectorEightInput { node_index: 3 }
            )
        );

        let candidate = cost_node(4, 0, 8, None, Some(0x84));
        service.answer = Err("selector unavailable");
        assert_eq!(
            check_assault_rule_budget(true, 1, 1, &[], &candidate, &mut service),
            AssaultBudgetDecision::Unavailable(AssaultBudgetUnavailable::SelectorEight(
                "selector unavailable"
            ))
        );
        assert_eq!(service.inputs, vec![0x84]);
    }

    #[test]
    fn assault_phase_availability_keeps_capacity_before_weighted_cost() {
        let mut evaluated_cost = false;
        assert_eq!(
            resolve_assault_phase_availability(1, true, || {
                evaluated_cost = true;
                AssaultBudgetDecision::<&str>::Exceeded { signed_total: 11 }
            }),
            AssaultPhaseAvailability::CapacityRefused
        );
        assert!(!evaluated_cost);
    }

    #[test]
    fn assault_phase_availability_only_disables_exactly_selected_word_one() {
        let mut evaluated_cost = false;
        assert_eq!(
            resolve_assault_phase_availability(2, true, || {
                evaluated_cost = true;
                AssaultBudgetDecision::<&str>::Exceeded { signed_total: 11 }
            }),
            AssaultPhaseAvailability::NotSelected
        );
        assert!(!evaluated_cost);

        assert_eq!(
            resolve_assault_phase_availability(u32::MAX, false, || {
                AssaultBudgetDecision::<&str>::Exceeded { signed_total: 11 }
            }),
            AssaultPhaseAvailability::NotSelected
        );
    }

    #[test]
    fn assault_phase_availability_preserves_budget_and_service_outcomes() {
        assert_eq!(
            resolve_assault_phase_availability(1, false, || {
                AssaultBudgetDecision::<&str>::Exceeded { signed_total: 11 }
            }),
            AssaultPhaseAvailability::BudgetRefused { signed_total: 11 }
        );
        assert_eq!(
            resolve_assault_phase_availability(1, false, || {
                AssaultBudgetDecision::<&str>::WithinBudget { signed_total: 10 }
            }),
            AssaultPhaseAvailability::Available
        );
        assert_eq!(
            resolve_assault_phase_availability(1, false, || {
                AssaultBudgetDecision::<&str>::Bypassed(AssaultBudgetBypass::NoActiveContext)
            }),
            AssaultPhaseAvailability::Available
        );
        assert_eq!(
            resolve_assault_phase_availability(1, false, || {
                AssaultBudgetDecision::Unavailable(AssaultBudgetUnavailable::SelectorEight(
                    "offline",
                ))
            }),
            AssaultPhaseAvailability::Unavailable(AssaultBudgetUnavailable::SelectorEight(
                "offline",
            ))
        );
    }

    #[test]
    fn assault_phase_ui_plan_refreshes_rows_for_non_refusal_results() {
        for availability in [
            AssaultPhaseAvailability::<&str>::NotSelected,
            AssaultPhaseAvailability::Available,
        ] {
            assert_eq!(
                plan_assault_phase_ui(availability, None),
                Ok(AssaultPhaseUiPlan::RefreshRowsThenWidget)
            );
        }
    }

    #[test]
    fn assault_phase_ui_plan_uses_descriptor_message_for_capacity_refusal() {
        let message = AssaultPhasePopupRequest {
            label_id: 0x0bb8,
            argument: 5,
        };
        assert_eq!(
            plan_assault_phase_ui(
                AssaultPhaseAvailability::<&str>::CapacityRefused,
                Some(message)
            ),
            Ok(AssaultPhaseUiPlan::PopupThenWidget(message))
        );
        assert_eq!(
            plan_assault_phase_ui(AssaultPhaseAvailability::<&str>::CapacityRefused, None),
            Err(AssaultPhaseUiPlanError::MissingCapacityMessage)
        );
    }

    #[test]
    fn assault_phase_ui_plan_preserves_fixed_budget_popup_and_unavailable_cost() {
        assert_eq!(
            plan_assault_phase_ui(
                AssaultPhaseAvailability::<&str>::BudgetRefused { signed_total: 11 },
                None,
            ),
            Ok(AssaultPhaseUiPlan::PopupThenWidget(
                AssaultPhasePopupRequest {
                    label_id: 0x0bba,
                    argument: 10,
                }
            ))
        );
        assert_eq!(
            plan_assault_phase_ui(
                AssaultPhaseAvailability::Unavailable(AssaultBudgetUnavailable::SelectorEight(
                    "offline",
                )),
                None,
            ),
            Err(AssaultPhaseUiPlanError::BudgetUnavailable(
                AssaultBudgetUnavailable::SelectorEight("offline")
            ))
        );
    }

    #[test]
    fn assault_rule_budget_uses_signed_wrapping_total_and_keeps_item_costs() {
        let overflow_candidate = cost_node(1, 0, 9, Some(u32::MAX), None);
        let mut service = TestAssaultCostService {
            inputs: Vec::new(),
            answer: Ok(0),
        };
        assert_eq!(
            check_assault_rule_budget(true, 1, 1, &[], &overflow_candidate, &mut service),
            AssaultBudgetDecision::WithinBudget { signed_total: -1 }
        );

        for selector in [1, 2, 9] {
            let candidate = cost_node(2, 0, selector, Some(11), None);
            assert_eq!(
                check_assault_rule_budget(true, 1, 1, &[], &candidate, &mut service),
                AssaultBudgetDecision::Exceeded { signed_total: 11 }
            );
        }
        let missing_item = cost_node(3, 0, 1, None, None);
        assert_eq!(
            check_assault_rule_budget(true, 1, 1, &[], &missing_item, &mut service),
            AssaultBudgetDecision::Unavailable(AssaultBudgetUnavailable::MissingItemCost {
                node_index: 3,
                selector: 1,
            })
        );
    }

    #[test]
    fn measured_preview_rows_resolve_distinct_variant_and_game_type_values() {
        let expected = [
            (0, 4),
            (2, 10),
            (3, 9),
            (4, 9),
            (5, 2),
            (6, 1),
            (7, 9),
            (8, 9),
            (9, 9),
            (10, 9),
            (12, 5),
            (15, 6),
            (16, 3),
            (18, 9),
            (19, 9),
        ];
        for (index, (variant, game_type)) in expected.into_iter().enumerate() {
            assert_eq!(
                PreviewSelection::from_row(PreviewRow(index as u8)),
                Some(PreviewSelection {
                    row: PreviewRow(index as u8),
                    variant: LaunchVariant(variant),
                    game_type: GameType(game_type),
                })
            );
        }
        assert_eq!(PreviewSelection::from_row(PreviewRow(15)), None);
    }
}
