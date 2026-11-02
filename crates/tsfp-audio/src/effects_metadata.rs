//! Bounded parser for the DirectSound effects image layout.
//!
//! This validates image-relative code/state slices and workspace-relative mappings only. Guest
//! memory access, image identity checks, decryption, DSP execution and binding remain at their
//! documented adapter boundaries; see `docs/t-audio-replace.md`.

pub const MAX_MAPS: usize = 9;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectMap {
    pub code_offset: u32,
    pub code_bytes: u32,
    pub state_offset: u32,
    pub state_bytes: u32,
    pub y_offset: u32,
    pub y_bytes: u32,
    pub workspace_offset: u32,
    pub workspace_bytes: u32,
}

/// C-compatible result layout for `dsound_effects_metadata`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub code_offset: u32,
    pub code_bytes: u32,
    pub state_offset: u32,
    pub state_bytes: u32,
    pub descriptor_offset: u32,
    pub descriptor_bytes: u32,
    pub iv_offset: u32,
    pub iv_bytes: u32,
    pub map_count: u32,
    pub workspace_bytes: u32,
    pub maps: [EffectMap; MAX_MAPS],
}

fn word(image: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = image.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn range(offset: u32, length: u32, start: u32, bytes: u32) -> bool {
    u64::from(offset) >= u64::from(start)
        && u64::from(offset) + u64::from(length) <= u64::from(start) + u64::from(bytes)
}

fn overlap(a: u32, a_bytes: u32, b: u32, b_bytes: u32) -> bool {
    a_bytes != 0
        && b_bytes != 0
        && u64::from(a) + u64::from(a_bytes) > u64::from(b)
        && u64::from(b) + u64::from(b_bytes) > u64::from(a)
}

/// Validate a guest-provided effects image layout. Refusal returns `None` without exposing a
/// partially parsed result. The source contract accepts zero-length aliases and requires every
/// nonempty code, state and workspace interval to fit and be pairwise disjoint within its class.
pub fn validate_layout(image: &[u8]) -> Option<Metadata> {
    if image.len() < 0x818 || image.len() > u32::MAX as usize {
        return None;
    }

    let code_words = u64::from(word(image, 0x804)?);
    let state_words = u64::from(word(image, 0x80c)?);
    let code_bytes = code_words.checked_mul(4)?;
    let state_bytes = state_words.checked_mul(4)?;
    if code_bytes > u64::from(u32::MAX) || state_bytes > u64::from(u32::MAX) {
        return None;
    }
    let code_bytes = code_bytes as u32;
    let state_bytes = state_bytes as u32;
    let state_offset = 0x818u64.checked_add(u64::from(code_bytes))?;
    let descriptor_offset = state_offset.checked_add(u64::from(state_bytes))?;
    if descriptor_offset.checked_add(8)? > image.len() as u64 {
        return None;
    }
    let state_offset = u32::try_from(state_offset).ok()?;
    let descriptor_offset = u32::try_from(descriptor_offset).ok()?;
    let map_count = word(image, descriptor_offset as usize)?;
    let workspace_bytes = word(image, descriptor_offset as usize + 4)?;
    let count = usize::try_from(map_count).ok()?;
    if count == 0 || count > MAX_MAPS || workspace_bytes == 0 {
        return None;
    }

    let descriptor_bytes = 8u32.checked_add(map_count.checked_mul(32)?)?;
    let iv_offset = u64::from(descriptor_offset).checked_add(u64::from(descriptor_bytes))?;
    let iv_bytes = map_count.checked_mul(8)?;
    if iv_offset.checked_add(u64::from(iv_bytes))? > image.len() as u64 {
        return None;
    }
    let iv_offset = u32::try_from(iv_offset).ok()?;
    let mut result = Metadata {
        code_offset: 0x818,
        code_bytes,
        state_offset,
        state_bytes,
        descriptor_offset,
        descriptor_bytes,
        iv_offset,
        iv_bytes,
        map_count,
        workspace_bytes,
        ..Metadata::default()
    };

    for index in 0..count {
        let offset = descriptor_offset as usize + 8 + index * 32;
        let mut words = [0u32; 8];
        for (word_index, value) in words.iter_mut().enumerate() {
            *value = word(image, offset + word_index * 4)?;
        }
        let workspace_address = words[6];
        let mut map = EffectMap {
            code_offset: words[0],
            code_bytes: words[1],
            state_offset: words[2],
            state_bytes: words[3],
            y_offset: words[4],
            y_bytes: words[5],
            workspace_offset: workspace_address.checked_sub(0xc000)?,
            workspace_bytes: words[7],
        };
        if !range(
            map.code_offset,
            map.code_bytes,
            result.code_offset,
            result.code_bytes,
        ) || !range(
            map.state_offset,
            map.state_bytes,
            result.state_offset,
            result.state_bytes,
        ) || map.y_offset != 0
            || map.y_bytes != 0
            || workspace_address < 0xc000
            || !range(
                map.workspace_offset,
                map.workspace_bytes,
                0,
                result.workspace_bytes,
            )
        {
            return None;
        }
        for previous in &result.maps[..index] {
            if overlap(
                map.code_offset,
                map.code_bytes,
                previous.code_offset,
                previous.code_bytes,
            ) || overlap(
                map.state_offset,
                map.state_bytes,
                previous.state_offset,
                previous.state_bytes,
            ) || overlap(
                map.workspace_offset,
                map.workspace_bytes,
                previous.workspace_offset,
                previous.workspace_bytes,
            ) {
                return None;
            }
        }
        map.workspace_offset = workspace_address - 0xc000;
        result.maps[index] = map;
    }
    Some(result)
}
