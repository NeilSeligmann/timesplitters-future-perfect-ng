//! Actor teardown at original 0x15DDA0. Resource helpers remain explicit leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupCounts {
    pub active: u32,
    pub removed: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DespawnCtx {
    pub object: u32,
    pub target: u32,
    pub record: u32,
    pub story_mask: u64,
    pub other_mask: u64,
    pub object_flags: u32,
    pub object_speed: f32,
    pub flags: u32,
    pub node: i32,
    pub name: [u8; 32],
    pub group_present: bool,
    pub group: GroupCounts,
    pub active_count: u32,
    pub story_mode: bool,
    pub record_event: u32,
    pub reserved_tag: Option<i16>,
    pub entity: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    Attachment,
    Message,
    ResourceA,
    ResourceB,
    Dormant,
    Reset,
    Story,
    RecordEvent(u32),
    Entity,
    CharacterLinks,
    TailResource,
    ObjectLinks,
    Object,
}
impl Release {
    pub fn original_address(self) -> u32 {
        match self {
            Self::Attachment => 0x379650,
            Self::Message => 0x15C310,
            Self::ResourceA => 0x15C160,
            Self::ResourceB => 0x15C120,
            Self::Dormant => 0x15BCC0,
            Self::Reset => 0xDAAE0,
            Self::Story => 0xE7CC0,
            Self::RecordEvent(_) => 0x160CF0,
            Self::Entity => 0x17BF70,
            Self::CharacterLinks => 0xD74D0,
            Self::TailResource => 0x10E1D0,
            Self::ObjectLinks => 0xDB590,
            Self::Object => 0xA9070,
        }
    }
}
pub trait DespawnWorld {
    /// Preserve helper side effects in ctx before the next stores/queries.
    fn release(&mut self, which: Release, ctx: &mut DespawnCtx);
    /// 0x9CC40(sign-extended reservation+8 short).
    fn resolve_node(&mut self, tag: i16) -> i32;
    fn node_flags(&mut self, node: i32) -> &mut u32;
    /// Handle-table slot (handle>>22), stride0x2E8, generation dword+0x2C.
    fn registered_handle(&mut self, handle: u32) -> u32;
}
pub fn despawn(ctx: &mut DespawnCtx, world: &mut impl DespawnWorld) {
    for call in [
        Release::Attachment,
        Release::Message,
        Release::ResourceA,
        Release::ResourceB,
        Release::Dormant,
    ] {
        world.release(call, ctx);
    }
    ctx.other_mask = 0;
    ctx.story_mask = 0;
    ctx.object_flags &= !0x61;
    ctx.object_speed = 0.0;
    world.release(Release::Reset, ctx);
    ctx.node = -1;
    // Original strncpy(C+50,empty_string,0x1E): bytes, not dwords.
    ctx.name[..30].fill(0);
    if ctx.group_present {
        let group = &mut ctx.group;
        ctx.active_count = ctx.active_count.wrapping_sub(1);
        group.active = group.active.wrapping_sub(1);
        group.removed = group.removed.wrapping_add(1);
        ctx.group_present = false;
    }
    if ctx.flags & 0x400000 != 0 {
        ctx.active_count = ctx.active_count.wrapping_sub(1);
    }
    if ctx.story_mode {
        world.release(Release::Story, ctx);
        if ctx.record_event != 0 {
            world.release(Release::RecordEvent(ctx.record_event), ctx);
        }
    }
    if let Some(tag) = ctx.reserved_tag {
        let node = world.resolve_node(tag);
        *world.node_flags(node) &= !0x40000000;
        ctx.reserved_tag = None;
    }
    if ctx.entity != 0 {
        if world.registered_handle(ctx.entity) == ctx.entity {
            world.release(Release::Entity, ctx);
        } else {
            ctx.entity = 0;
        }
    }
    ctx.record = 0;
    world.release(Release::CharacterLinks, ctx);
    ctx.target = 0;
    world.release(Release::TailResource, ctx);
    world.release(Release::ObjectLinks, ctx);
    world.release(Release::Object, ctx);
    ctx.object = 0;
}
