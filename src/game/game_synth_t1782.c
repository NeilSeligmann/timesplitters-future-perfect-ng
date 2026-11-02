/* SPDX-License-Identifier: GPL-3.0-or-later */
/* T1782: synthesized-domain fault-only roots, group B (docs/t1773-synth-group-b.md).
 * Admitted bodies are the frozen drafts, concatenated unmodified. Do not edit by hand:
 * regenerate with `python -m tools.t1782_register`. */
#include "game_replace.h"
extern __thread uint32_t g_ebx, g_ebp, g_edi;
extern __thread int g_df;
static inline uint16_t read16(uint32_t a) { uint16_t v; memcpy(&v, game_host_ptr(a), 2u); return v; }
static inline void write16(uint32_t a, uint16_t v) { memcpy(game_host_ptr(a), &v, 2u); }

/* Original 0x00109990: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00109990, cdecl, 0, u32, game_pool_73bd40_collect_types_0x19_0x1a)
{
    g_eax = guest_read32(0x73BD40u);
    g_edx = 0u;
    guest_write32(g_esp - 4u, g_esi);
    guest_write32(0x73B920u, g_edx);
    g_esi = 100u;
    do {
        g_ecx = guest_read32(g_eax);
        if ((int32_t)g_ecx >= 25 && (int32_t)g_ecx <= 26) {
            guest_write32(0x73BBB0u + g_edx * 4u, g_eax);
            ++g_edx;
        }
        g_eax += 0xD0u;
        --g_esi;
    } while (g_esi != 0u);
    guest_write32(0x73B920u, g_edx);
    guest_write32(0x4E60E0u, 0u);
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x001118A0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001118A0, cdecl, 4, u32, game_global_73bd68_set_four_vec3_0x9c_to_0xc8)
{
    g_eax = guest_read32(0x73BD68u);
    for (unsigned i = 0u; i < 4u; ++i) {
        g_ecx = game_stack_arg(i);
        g_edx = guest_read32(g_ecx);
        guest_write32(g_eax + 0x9Cu + 12u * i, g_edx);
        g_edx = guest_read32(g_ecx + 4u);
        guest_write32(g_eax + 0xA0u + 12u * i, g_edx);
        g_ecx = guest_read32(g_ecx + 8u);
        guest_write32(g_eax + 0xA4u + 12u * i, g_ecx);
    }
}

/* Original 0x00119C20: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00119C20, cdecl, 0, u32, game_global_7ac804_current_entry_get_field_0)
{
    g_eax = guest_read32(0x7AC808u);
    g_ecx = guest_read32(0x7AC804u);
    g_eax *= 5u;
    g_eax = guest_read32(g_ecx + g_eax * 8u);
}

/* Original 0x00139160: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00139160, cdecl, 0, u32, game_hud_local_player_slot_state_not_0xc_or_0x11_and_flag_0x40_clear)
{
    g_ecx = guest_read32(0x7B0C7Cu);
    g_eax = guest_read32(g_ecx + 4u);
    if ((int32_t)g_eax < 0) { g_eax = 0u; return; }
    g_edx = (g_edx & 0xFFFFFF00u) | guest_read8(g_ecx + 12u);
    g_eax = g_eax * 0x177Cu + 0x7A6160u;
    if ((g_edx & 0x40u) != 0u) { g_eax = 0u; return; }
    g_eax = guest_read32(g_eax);
    g_eax = (g_eax == 12u || g_eax == 17u) ? 0u : 1u;
}

/* Original 0x00142990: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00142990, cdecl, 0, u32, game_current_slot_7a6160_set_field_0x1750_3)
{
    g_eax = guest_read32(0x7B0C7Cu);
    g_eax = guest_read32(g_eax + 4u);
    if ((int32_t)g_eax >= 0) {
        g_eax *= 0x177Cu;
        guest_write32(g_eax + 0x7A78B0u, 3u);
    }
}

/* Original 0x00147080: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00147080, cdecl, 13, u32, game_player_input_binding_or_button_mask_get)
{
    g_eax = game_stack_arg(0u);
    g_esp -= 0x38u;
    if ((int32_t)g_eax < 14) {
        g_ecx = guest_read32(g_esp + 0x40u);
        g_edx = guest_read32(0x7B0C48u);
        g_ecx = g_ecx * 0x561u + g_eax;
        g_eax = guest_read32(g_edx + g_ecx * 4u + 24u);
    } else {
        static const uint32_t masks[14] = {
            1u,2u,4u,8u,16u,32u,64u,128u,0x8000u,0x400u,0x4000u,0x800u,12u,0u
        };
        for (unsigned i = 0u; i < 14u; ++i) guest_write32(g_esp + i * 4u, masks[i]);
        g_eax = guest_read32(g_esp + g_eax * 4u - 0x3Cu);
    }
    g_esp += 0x38u;
}

/* Original 0x00156DB0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00156DB0, cdecl, 4, u32, game_node_pool_init_freelist)
{
    g_eax = game_stack_arg(3u);
    g_edx = game_stack_arg(2u);
    guest_write32(g_esp - 4u, g_esi);
    g_esi = game_stack_arg(0u);
    g_ecx = 0u;
    uint32_t count = g_eax;
    guest_write32(g_esi, g_edx);
    guest_write32(g_esi + 8u, g_eax);
    guest_write32(g_esi + 12u, g_eax);
    if (count != 0u) {
        guest_write32(g_esp - 8u, g_ebx);
        g_ebx = game_stack_arg(1u);
        guest_write32(g_esp - 12u, g_edi);
        g_edi = count;
        do {
            g_eax = g_edx;
            g_edx += g_ebx;
            --g_edi;
            guest_write32(g_eax, g_ecx);
            g_ecx = g_eax;
        } while (g_edi != 0u);
        g_edi = guest_read32(g_esp - 12u);
        g_ebx = guest_read32(g_esp - 8u);
    }
    guest_write32(g_esi + 4u, g_ecx);
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x00165760: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00165760, cdecl, 0, u32, game_entry_pool_74c458_reset_free_id_lists_clear_slot_table_and_reset_cursors)
{
    g_ecx = guest_read32(0x74C49Cu);
    g_eax = 0x58u;
    write16(0x74C45Cu, (uint16_t)g_eax);
    write16(0x74C45Eu, (uint16_t)g_eax);
    g_eax = g_ecx + 0x60u;
    g_ecx &= 0xFFFFu;
    guest_write32(g_esp - 4u, g_ebx);
    write16(0x74C460u, (uint16_t)g_eax);
    g_ecx += 0x60u;
    g_ebx = 0u; g_eax = 0u;
    if ((int32_t)g_ecx > 0) {
        g_ecx = guest_read32(0x74C46Cu);
        g_edx = guest_read32(0x74C480u);
        guest_write32(g_esp - 8u, g_ebp);
        guest_write32(g_esp - 12u, g_esi);
        g_esi = guest_read32(0x74C494u);
        guest_write32(g_esp - 16u, g_edi);
        g_edi = guest_read32(0x74C488u);
        g_edx -= g_ecx; g_esi -= g_ecx;
        do {
            write16(g_edi + g_eax * 2u, (uint16_t)g_eax);
            g_ebp = g_eax | 0x10000u;
            guest_write32(g_edx + g_ecx, g_ebp);
            guest_write32(g_ecx, g_ebx);
            guest_write32(g_esi + g_ecx, g_ebx);
            g_ebp = read16(0x74C49Cu);
            ++g_eax; g_ebp += 0x60u; g_ecx += 4u;
        } while ((int32_t)g_eax < (int32_t)g_ebp);
        g_edi = guest_read32(g_esp - 16u);
        g_esi = guest_read32(g_esp - 12u);
        g_ebp = guest_read32(g_esp - 8u);
    }
    g_eax = guest_read32(0x74C464u) + 0x78u;
    g_ecx = 0x5Du;
    do {
        guest_write32(g_eax - 0x20u, g_ebx);
        guest_write32(g_eax, g_ebx);
        guest_write32(g_eax + 4u, g_ebx);
        guest_write32(g_eax - 8u, g_ebx);
        guest_write32(g_eax - 4u, g_ebx);
        g_eax += 0x80u; --g_ecx;
    } while (g_ecx != 0u);
    g_edx = guest_read32(0x74C478u);
    g_eax = 0xFFFFFFFFu;
    guest_write32(0x74C484u, g_ebx);
    guest_write32(0x74C474u, g_ebx);
    guest_write32(0x74C47Cu, g_ebx);
    guest_write32(0x74C490u, g_ebx);
    write16(0x74C470u, (uint16_t)g_ebx);
    guest_write32(0x74C450u, g_eax);
    guest_write32(0x74C468u, g_edx);
    guest_write32(0x4E826Cu, g_eax);
    g_ebx = guest_read32(g_esp - 4u);
}

/* Original 0x00167D20: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00167D20, cdecl, 2, u32, game_collision_shape_records_have_active_or_match_table_51c088_index)
{
    g_ecx = game_stack_arg(1u);
    g_eax = game_stack_arg(0u);
    guest_write32(g_esp - 4u, g_esi);
    if (g_ecx == 0xFFFFFFFFu) {
        g_ecx = guest_read32(g_eax); g_edx = 0u;
        if ((int32_t)g_ecx > 0) {
            g_esi = g_ecx * 0xC8u;
            g_eax = g_eax - g_esi + 0x8Cu;
            do {
                if (read16(g_eax - 0x34u) != 0u && guest_read32(g_eax) != 0u) {
                    g_eax = 1u; goto done;
                }
                ++g_edx; g_eax += 0xC8u;
            } while ((int32_t)g_edx < (int32_t)g_ecx);
        }
    } else {
        g_edx = guest_read32(g_eax);
        g_esi = guest_read32(g_ecx * 8u + 0x51C08Cu);
        g_edx *= 0xC8u; g_eax -= g_edx;
        g_edx = guest_read32(g_eax + 0x8Cu);
        if (g_edx == g_esi) {
            g_eax = (uint32_t)(int32_t)(int16_t)read16(g_eax + 0x58u);
            if (g_eax == guest_read32(g_ecx * 8u + 0x51C088u)) {
                g_eax = 1u; goto done;
            }
        }
    }
    g_eax = 0u;
done:
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x0016B170: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0016B170, cdecl, 1, u32, game_table_74c464_slot_is_used)
{
    g_eax = game_stack_arg(0u);
    g_edx = guest_read32(0x74C464u);
    g_eax <<= 7;
    guest_write32(g_esp - 4u, g_esi);
    g_esi = guest_read32(g_eax + g_edx + 0x58u);
    g_ecx = g_esi != 0u;
    g_esi = guest_read32(g_esp - 4u);
    g_eax = g_ecx;
}

/* Original 0x0016B190: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0016B190, cdecl, 1, u32, game_table_74c464_free_slot)
{
    g_eax = game_stack_arg(0u);
    g_ecx = guest_read32(0x74C464u);
    g_eax = (g_eax << 7) + g_ecx;
    write16(0x74C470u, (uint16_t)(read16(0x74C470u) - 1u));
    g_ecx = 0u;
    guest_write32(g_eax + 0x58u, g_ecx);
    guest_write32(g_eax + 0x78u, g_ecx);
    guest_write32(g_eax + 0x7Cu, g_ecx);
}

/* Original 0x0016B7C0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0016B7C0, cdecl, 2, u32, game_table_74c464_slot_set_field_0x7c)
{
    g_eax = game_stack_arg(0u); g_ecx = game_stack_arg(1u);
    g_edx = guest_read32(0x74C464u); g_eax <<= 7;
    guest_write32(g_eax + g_edx + 0x7Cu, g_ecx);
}

/* Original 0x0017F780: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0017F780, cdecl, 0, u32, game_table_74c510_restore_from_74c514_set_count_74c66c)
{
    g_eax = guest_read32(0x74C59Cu);
    guest_write32(g_esp - 4u, g_esi);
    g_esi = guest_read32(0x74C514u);
    guest_write32(g_esp - 8u, g_edi);
    g_edi = guest_read32(0x74C510u); g_ecx = 0x5Au;
    do {
        uint32_t value = guest_read32(g_esi);
        guest_write32(g_edi, value);
        uint32_t step = g_df ? 0xFFFFFFFCu : 4u;
        g_esi += step; g_edi += step; --g_ecx;
    } while (g_ecx != 0u);
    g_edi = guest_read32(g_esp - 8u);
    guest_write32(0x74C66Cu, g_eax);
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x00183040: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00183040, cdecl, 1, u32, game_set_record_24_word_2_to_1_by_index_74c6f8)
{
    g_eax = game_stack_arg(0u); g_ecx = guest_read32(0x74C6F8u);
    g_eax *= 3u; write16(g_ecx + g_eax * 8u + 2u, 1u);
}

/* Original 0x001B5630: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B5630, cdecl, 1, u32, game_table_53a3dc_stride_0x30_entry_15_minus_field_div_3)
{
    g_eax = game_stack_arg(0u) * 3u;
    g_eax <<= 4; g_edx = guest_read32(g_eax + 0x53A3DCu);
    g_ecx = 15u - g_edx;
    int64_t product = (int64_t)0x55555556 * (int64_t)(int32_t)g_ecx;
    g_edx = (uint32_t)((uint64_t)product >> 32);
    g_eax = (g_edx >> 31) + g_edx;
}

/* Original 0x001B5660: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B5660, cdecl, 2, u32, game_select_entry_stride_0x7c_at_offset_0x68_or_first_if_id_negative)
{
    g_eax = game_stack_arg(1u); g_ecx = game_stack_arg(0u);
    g_eax *= 0x7Cu; g_edx = guest_read32(g_eax + g_ecx + 0x68u);
    g_eax += g_ecx + 0x68u;
    if ((int32_t)g_edx < 0) g_eax = g_ecx + 0x68u;
}

/* Original 0x001B66B0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B66B0, cdecl, 1, u32, game_slot_4fa128_walk_back_to_chain_head_if_flag_4)
{
    g_eax = game_stack_arg(0u); g_ecx = g_eax * 0x3Cu;
    if ((guest_read8(g_ecx + 0x4FA128u) & 4u) != 0u) {
        --g_eax; g_edx = g_eax * 0x3Cu; g_ecx = g_edx + 0x4FA128u;
        if ((int8_t)guest_read8(g_ecx) >= 0) {
            do {
                g_edx = (g_edx & 0xFFFFFF00u) | guest_read8(g_ecx - 0x3Cu);
                g_ecx -= 0x3Cu; --g_eax;
            } while ((int8_t)(uint8_t)g_edx >= 0);
        }
    }
}

/* Original 0x001B66E0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B66E0, cdecl, 5, u32, game_ammo_decrement_word_counter_clamp_zero_and_inc_shots_0x10c)
{
    g_eax = game_stack_arg(1u);
    guest_write32(g_eax + 0x10Cu, guest_read32(g_eax + 0x10Cu) + 1u);
    g_ecx = game_stack_arg(4u); g_edx = 0u;
    guest_write32(g_esp - 4u, g_esi);
    uint32_t displacement = 0x38u;
    if (g_ecx == 0u || g_ecx == 1u) { g_eax = game_stack_arg(0u); displacement = 0x61Eu; }
    g_ecx = game_stack_arg(2u);
    g_esi = (g_esi & 0xFFFF0000u) | (uint16_t)game_stack_arg(3u);
    uint32_t field = g_eax + g_ecx * 2u + displacement;
    write16(field, (uint16_t)(read16(field) - (uint16_t)g_esi));
    if ((int16_t)read16(field) < 0) write16(field, (uint16_t)g_edx);
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x001B6740: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B6740, cdecl, 1, u32, game_object_field_0x44_set_and_current_player_flag_4000_or_mode_0xa_record_flag_0x40)
{
    g_eax = game_stack_arg(0u); g_ecx = guest_read32(g_eax + 0x44u);
    if (g_ecx != 0u) {
        g_eax = guest_read32(g_eax + 0x30u); g_ecx = guest_read32(0x4F9BACu);
        g_eax = g_eax * 0x29Cu + g_ecx;
        g_ecx = guest_read32(0x7B0C7Cu); g_edx = guest_read32(g_ecx + 12u);
        if ((g_edx & 0x4000u) != 0u) { g_eax = 1u; return; }
        if (guest_read8(0x7DE455u) == 10u && (guest_read8(g_eax + 0x68u) & 0x40u) != 0u) {
            g_eax = 1u; return;
        }
    }
    g_eax = 0u;
}

/* Original 0x001B6C50: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B6C50, cdecl, 1, u32, game_current_player_object_0x44_matches_selected_slot_204_vs_table_4fa138)
{
    g_eax = guest_read32(0x7B0C7Cu); g_ecx = guest_read32(g_eax + 0x14u);
    g_eax = guest_read32(g_ecx + 0x7Cu); g_ecx = guest_read32(g_eax + 0x80u);
    if (g_ecx == 0u) {
        g_edx = game_stack_arg(0u); g_ecx = guest_read32(g_edx + 0x44u);
        if (g_ecx != 0u) {
            g_ecx = guest_read32(g_eax + 0x94u); g_edx = guest_read32(g_eax + 0x204u);
            g_ecx *= 0x3Cu;
            if (g_edx == guest_read32(g_ecx + 0x4FA138u)) { g_eax = 1u; return; }
        }
    }
    g_eax = 0u;
}

/* Original 0x001B7CC0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B7CC0, cdecl, 1, u32, game_object_has_record_29c_with_valid_id_and_current_player_ready)
{
    g_eax = game_stack_arg(0u); g_eax = guest_read32(g_eax + 0x30u);
    if (g_eax != 0xFFFFFFFFu) {
        g_ecx = guest_read32(0x7B0C7Cu); g_edx = guest_read32(g_ecx + 4u);
        if ((int32_t)g_edx >= 0) {
            g_edx = guest_read32(g_ecx + 0xE84u);
            if (g_edx != 0u) {
                g_ecx = guest_read32(0x4F9BACu); g_eax *= 0x29Cu;
                if (guest_read32(g_eax + g_ecx) != 0xFFFFFFFFu) { g_eax = 1u; return; }
            }
        }
    }
    g_eax = 0u;
}

/* Original 0x001B90F0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001B90F0, cdecl, 0, u32, game_flags_7de458_0x60000000_clear_and_object_7b0cc0_flag_0x21c_bit2_set)
{
    if ((guest_read32(0x7DE458u) & 0x60000000u) == 0u) {
        g_eax = guest_read32(0x7B0CC0u);
        if ((guest_read8(g_eax + 0x21Cu) & 4u) != 0u) { g_eax = 1u; return; }
    }
    g_eax = 0u;
}

/* Original 0x001BA130: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001BA130, cdecl, 2, u32, game_object_set_current_weapon_slot_0x94_and_store_order_byte_0xe3c)
{
    g_eax = game_stack_arg(1u); g_ecx = g_eax * 0x3Cu;
    uint8_t flags = guest_read8(g_ecx + 0x4FA128u);
    g_edx = game_stack_arg(0u);
    guest_write32(g_esp - 4u, g_ebx);
    if ((flags & 4u) != 0u) {
        g_ecx = guest_read32(g_edx + 0x80u);
        if (g_ecx != 0u) {
            g_ecx = guest_read32(0x7DE458u);
            if ((g_ecx & 0x1000u) == 0u) {
                --g_eax; g_ecx = g_eax * 0x3Cu;
                g_ebx = (g_ebx & 0xFFFFFF00u) | guest_read8(g_ecx + 0x4FA128u);
                g_ecx += 0x4FA128u;
                if ((int8_t)(uint8_t)g_ebx >= 0) {
                    do {
                        g_ebx = (g_ebx & 0xFFFFFF00u) | guest_read8(g_ecx - 0x3Cu);
                        g_ecx -= 0x3Cu; --g_eax;
                    } while ((int8_t)(uint8_t)g_ebx >= 0);
                }
            }
        }
    }
    g_ecx = guest_read32(g_edx + 8u);
    guest_write32(g_edx + 0x94u, g_eax);
    if ((int32_t)g_ecx < (int32_t)guest_read32(0x790950u)) {
        g_edx = g_eax * 0x3Cu; g_ecx *= 0x1584u;
        g_ebx = guest_read32(g_edx + 0x4FA160u); g_edx = guest_read32(0x7B0C48u);
        g_ecx += g_ebx; guest_write8(g_ecx + g_edx + 0xE3Cu, (uint8_t)g_eax);
    }
    g_ebx = guest_read32(g_esp - 4u);
}

/* Original 0x001C8C20: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(001C8C20, cdecl, 8, u32, game_player_object_hit_reaction_allowed_by_anim_flag_bits_and_field_0x124)
{
    g_edx = game_stack_arg(0u); g_eax = guest_read32(g_edx + 0x20u);
    g_esp -= 8u;
    guest_write32(g_esp - 4u, g_esi); guest_write32(g_esp - 8u, g_edi);
    g_edi = guest_read32(g_esp + 0x10u);
    if (g_eax == 8u) {
        g_esi = guest_read32(g_edx + 0x7Cu);
        if ((guest_read32(g_esi + 0x28u) & 0x800000u) != 0u) {
            g_eax = (uint32_t)(int32_t)(int16_t)read16(g_edi * 2u + 0x7E03C0u);
            g_eax = guest_read32(g_eax * 8u + 0x7DE660u) & 0x1000028u;
            g_ecx = 0u;
            if (g_eax == 0u) goto early_zero;
        }
        g_ecx = guest_read32(g_edx + 4u); g_eax = guest_read32(g_ecx + 0xD4u);
        if (g_eax != 0u) {
            g_eax = (uint32_t)(int32_t)(int16_t)read16(g_eax * 2u + 0x7E03C0u);
            g_eax = guest_read32(g_eax * 8u + 0x7DE660u) & 0x20u; g_ecx = 0u;
            if (g_eax != 0u) {
                g_ecx = guest_read32(g_esi + 0x14u) & 1u; g_eax = g_ecx;
                if (g_eax == 0u) goto early_zero;
            }
        }
    }
    guest_write32(g_esp - 12u, g_ebx);
    g_ebx = guest_read32(g_edx + 4u); g_esi = guest_read32(g_ebx + 0xD4u);
    if (g_esi != 0u) {
        g_edx = (uint32_t)(int32_t)(int16_t)read16(g_edi * 2u + 0x7E03C0u);
        g_ecx = guest_read32(g_edx * 8u + 0x7DE660u);
        g_eax = guest_read32(g_edx * 8u + 0x7DE664u);
        guest_write32(g_esp + 4u, g_eax);
        g_eax = g_ecx & 0x20u; g_edx = 0u;
        if (g_eax == 0u) {
            g_edx = (uint32_t)(int32_t)(int16_t)read16(g_esi * 2u + 0x7E03C0u);
            g_eax = guest_read32(g_edx * 8u + 0x7DE660u) & 0x1C00u; g_edx = 0u;
            if (g_eax != 0u) {
                g_ecx &= 0x1000000u; g_eax = 0u;
                if (g_ecx == 0u) {
                    g_eax = guest_read32(g_ebx + 0x124u);
                    if (g_eax == 0u && (guest_read8(g_ebx + 0x128u) & 4u) == 0u) goto late_zero;
                }
            }
        }
    }
    g_eax = (uint32_t)(int32_t)(int16_t)read16(g_edi * 2u + 0x7E03C0u);
    g_ecx = guest_read32(g_eax * 8u + 0x7DE660u);
    g_edx = guest_read32(g_eax * 8u + 0x7DE664u);
    g_eax = g_ecx & 0x20u; guest_write32(g_esp + 4u, g_edx); g_edx = 0u;
    if (g_eax == 0u && g_esi != 0u) {
        g_ecx &= 8u; g_eax = 0u;
        if (g_ecx == 0u) {
            g_eax = (uint32_t)(int32_t)(int16_t)read16(g_esi * 2u + 0x7E03C0u);
            g_eax = guest_read32(g_eax * 8u + 0x7DE660u) & 8u; g_ecx = 0u;
            if (g_eax != 0u) {
                g_eax = guest_read32(g_ebx + 0x124u);
                if (g_eax == 0u) goto late_zero;
            }
        }
    }
    g_eax = 1u; goto late_done;
late_zero:
    g_eax = 0u;
late_done:
    g_ebx = guest_read32(g_esp - 12u);
    goto restore;
early_zero:
    g_eax = 0u;
restore:
    g_edi = guest_read32(g_esp - 8u); g_esi = guest_read32(g_esp - 4u); g_esp += 8u;
}

/* Original 0x0020FA50: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0020FA50, cdecl, 2, u32, game_object_75c0a0_replace_matching_handle_fields_0x43c_to_0x448)
{
    g_ecx = guest_read32(0x75C0A0u); g_edx = guest_read32(g_ecx + 0x43Cu);
    guest_write32(g_esp - 4u, g_ebx); guest_write32(g_esp - 8u, g_ebp);
    guest_write32(g_esp - 12u, g_esi); g_esi = 0u;
    guest_write32(g_esp - 16u, g_edi);
    if (g_edx != 0u) {
        g_ebx = guest_read32(0x72F5E8u);
        g_eax = (g_edx >> 22) * 0x2E8u;
        g_edi = guest_read32(g_eax + g_ebx + 0x2Cu); g_eax += g_ebx;
        if (g_edi != g_edx) { guest_write32(g_ecx + 0x43Cu, g_esi); g_eax = 0u; }
    } else g_eax = 0u;
    g_ebx = game_stack_arg(0u); g_edi = game_stack_arg(1u);
    if (g_eax == g_ebx) {
        g_eax = guest_read32(g_edi + 0x2Cu); guest_write32(g_ecx + 0x43Cu, g_eax);
    }
    for (uint32_t offset = 0x440u; offset <= 0x448u; offset += 4u) {
        g_edx = guest_read32(g_ecx + offset);
        if (g_edx != g_esi) {
            g_ebp = guest_read32(0x72F5E8u);
            g_eax = (g_edx >> 22) * 0x2E8u + g_ebp;
            if (guest_read32(g_eax + 0x2Cu) != g_edx) {
                guest_write32(g_ecx + offset, g_esi); g_eax = 0u;
            }
        } else g_eax = 0u;
        if (g_eax == g_ebx) {
            if (offset == 0x444u) {
                g_eax = guest_read32(g_edi + 0x2Cu); guest_write32(g_ecx + offset, g_eax);
            } else {
                g_edx = guest_read32(g_edi + 0x2Cu); guest_write32(g_ecx + offset, g_edx);
            }
        }
    }
    g_edi = guest_read32(g_esp - 16u); g_esi = guest_read32(g_esp - 12u);
    g_ebp = guest_read32(g_esp - 8u); g_ebx = guest_read32(g_esp - 4u);
}

/* Original 0x00227400: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00227400, cdecl, 1, u32, game_is_slot_4c_status_above_1_by_index)
{
    g_eax = game_stack_arg(0u); g_ecx = guest_read32(0x78CECCu);
    g_eax = g_eax * 0x4Cu + g_ecx;
    g_eax = (g_eax & 0xFFFFFF00u) | guest_read8(g_eax + 11u);
    uint8_t status = (uint8_t)g_eax; g_eax = status != 0u && status != 1u;
}

/* Original 0x002275F0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(002275F0, cdecl, 1, u32, game_slot_4c_set_status_5_if_status_2_or_3)
{
    g_eax = game_stack_arg(0u); g_ecx = guest_read32(0x78CECCu);
    g_eax = g_eax * 0x4Cu + g_ecx;
    g_ecx = (g_ecx & 0xFFFFFF00u) | guest_read8(g_eax + 11u);
    if ((uint8_t)g_ecx == 2u || (uint8_t)g_ecx == 3u) guest_write8(g_eax + 11u, 5u);
}

/* Original 0x00227620: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00227620, cdecl, 1, u32, game_slot_4c_demote_status_2_or_3_to_1_decrement_type_count)
{
    g_eax = game_stack_arg(0u); g_ecx = guest_read32(0x78CECCu);
    g_eax = g_eax * 0x4Cu + g_ecx;
    g_ecx = (g_ecx & 0xFFFFFF00u) | guest_read8(g_eax + 11u);
    if ((uint8_t)g_ecx == 2u || (uint8_t)g_ecx == 3u) {
        guest_write8(g_eax + 11u, 1u); g_eax = guest_read8(g_eax + 8u);
        uint32_t counter = g_eax * 4u + 0x75C134u;
        guest_write32(counter, guest_read32(counter) - 1u);
    }
}

/* Original 0x00227880: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00227880, cdecl, 1, u32, game_slot_4c_current_add_unique_dword_to_list_of_10_at_data_plus_4)
{
    g_eax = guest_read32(0x78CED0u); g_ecx = guest_read32(0x78CECCu);
    g_eax *= 0x4Cu; g_edx = guest_read32(g_eax + g_ecx + 12u);
    guest_write32(g_esp - 4u, g_esi); g_esi = guest_read32(g_edx);
    guest_write32(g_esp - 8u, g_edi); g_edi = guest_read32(g_edx + 4u);
    g_ecx = g_edx + 4u; g_eax = 0u;
    uint32_t first = g_edi; g_edi = game_stack_arg(0u);
    if (first != g_esi) {
        do {
            if (guest_read32(g_ecx) == g_edi) goto done;
            ++g_eax; g_ecx += 4u;
            if ((int32_t)g_eax >= 10) goto done;
        } while (guest_read32(g_ecx) != g_esi);
    }
    guest_write32(g_edx + g_eax * 4u + 4u, g_edi);
done:
    g_edi = guest_read32(g_esp - 8u); g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x002278E0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(002278E0, cdecl, 1, u32, game_slot_4c_current_add_unique_id_to_minus1_terminated_list_at_0x10)
{
    g_ecx = guest_read32(0x78CED0u) * 0x4Cu;
    guest_write32(g_esp - 4u, g_ebx); guest_write32(g_esp - 8u, g_esi);
    g_ecx += guest_read32(0x78CECCu); g_edx = guest_read32(g_ecx + 0x10u);
    g_eax = g_ecx + 0x10u; g_esi = 0u;
    guest_write32(g_esp - 12u, g_edi); g_edi = game_stack_arg(0u);
    if (g_edx != 0xFFFFFFFFu) {
        g_edx = g_eax;
        do {
            if (guest_read32(g_edx) == g_edi) goto done;
            g_ebx = guest_read32(g_eax + 4u); g_eax += 4u; ++g_esi; g_edx = g_eax;
        } while (g_ebx != 0xFFFFFFFFu);
    }
    guest_write32(g_ecx + g_esi * 4u + 0x10u, g_edi);
done:
    g_edi = guest_read32(g_esp - 12u); g_esi = guest_read32(g_esp - 8u);
    g_ebx = guest_read32(g_esp - 4u);
}

/* Original 0x00227980: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00227980, cdecl, 5, u32, game_slot_4c_lookup_dword_in_data_table_at_0x54_by_id_and_sub_index)
{
    g_eax = game_stack_arg(0u); g_ecx = guest_read32(0x78CECCu); g_eax *= 0x4Cu;
    guest_write32(g_esp - 4u, g_esi); g_esi = guest_read32(g_eax + g_ecx + 12u);
    guest_write32(g_esp - 8u, g_edi); g_edi = game_stack_arg(1u);
    g_ecx = 0u; g_edx = 0u; g_eax = g_esi + 0x2Cu;
    do {
        if (g_edi == guest_read32(g_eax - 0x28u)) {
            g_edx = game_stack_arg(2u); g_ecx += g_edx;
            g_eax = guest_read32(g_esi + g_ecx * 4u + 0x54u);
            goto done;
        }
        g_ecx += guest_read32(g_eax); ++g_edx; g_eax += 4u;
    } while ((int32_t)g_edx < 10);
    g_eax = 0u;
done:
    g_edi = guest_read32(g_esp - 8u); g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x00227A30: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00227A30, cdecl, 0, u32, game_clear_byte_48_of_current_slot_4c)
{
    g_eax = guest_read32(0x78CED0u); g_ecx = guest_read32(0x78CECCu);
    g_eax *= 0x4Cu; guest_write8(g_eax + g_ecx + 0x48u, 0u);
}

/* Original 0x00227A50: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00227A50, cdecl, 1, u32, game_set_dword_3c_of_current_slot_4c_from_arg)
{
    g_eax = guest_read32(0x78CED0u); g_ecx = game_stack_arg(0u);
    g_eax *= 0x4Cu; g_edx = guest_read32(0x78CECCu);
    guest_write32(g_eax + g_edx + 0x3Cu, g_ecx);
}

/* Original 0x00228FB0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(00228FB0, cdecl, 0, u32, game_zero_fill_buffer_75cc8c_and_set_flag_0x8000_if_state_7de470_4)
{
    g_edx = guest_read32(0x75CC8Cu);
    guest_write32(g_esp - 4u, g_edi);
    g_eax = 0u; g_ecx = 0x675u; g_edi = g_edx;
    do {
        guest_write32(g_edi, g_eax); g_edi += g_df ? 0xFFFFFFFCu : 4u; --g_ecx;
    } while (g_ecx != 0u);
    g_ecx = 0x672u; g_edi = g_edx + 12u;
    do {
        guest_write32(g_edi, g_eax); g_edi += g_df ? 0xFFFFFFFCu : 4u; --g_ecx;
    } while (g_ecx != 0u);
    uint32_t mode = guest_read32(0x7DE470u);
    g_edi = guest_read32(g_esp - 4u);
    if (mode == 4u) {
        g_eax = guest_read32(0x7B0C48u);
        guest_write32(g_eax + 12u, guest_read32(g_eax + 12u) | 0x8000u);
    }
}

/* Original 0x0022BDC0: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0022BDC0, cdecl, 0, u32, game_get_3_minus_dword_28_of_object_75cc98)
{
    g_ecx = guest_read32(0x75CC98u); g_edx = guest_read32(g_ecx + 0x28u);
    g_eax = 3u - g_edx;
}

/* Original 0x0022D110: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0022D110, cdecl, 3, u32, game_get_text_line_ptrs_75c770_and_75c370_by_index_null_if_empty)
{
    g_edx = game_stack_arg(2u); g_ecx = game_stack_arg(0u); g_eax = g_edx * 0xFFu;
    guest_write32(g_esp - 4u, g_esi); g_esi = game_stack_arg(1u);
    guest_write32(g_ecx, 0u); g_eax += 0x75C770u; guest_write32(g_esi, 0u);
    if (guest_read8(g_eax) != 0u) guest_write32(g_ecx, g_eax);
    g_ecx = guest_read32(0x7B0C7Cu); g_eax = guest_read32(g_ecx + 4u);
    g_eax = (g_edx + g_eax * 2u) * 0xFFu;
    g_ecx = (g_ecx & 0xFFFFFF00u) | guest_read8(g_eax + 0x75C370u);
    g_eax += 0x75C370u;
    if ((uint8_t)g_ecx != 0u) guest_write32(g_esi, g_eax);
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x0022D410: ordered reads/writes and actual saved guest slots. */
GAME_REPLACE_EXACT(0022D410, cdecl, 2, u32, game_get_text_line_ptrs_75c988_and_75ca88_null_if_empty)
{
    g_eax = game_stack_arg(0u); g_ecx = game_stack_arg(1u);
    guest_write32(g_eax, 0u); guest_write32(g_ecx, 0u);
    g_edx = (g_edx & 0xFFFFFF00u) | guest_read8(0x75C988u);
    if ((uint8_t)g_edx != 0u) guest_write32(g_eax, 0x75C988u);
    g_eax = guest_read32(0x7B0C7Cu); g_eax = guest_read32(g_eax + 4u) * 0xFFu;
    g_edx = (g_edx & 0xFFFFFF00u) | guest_read8(g_eax + 0x75CA88u);
    g_eax += 0x75CA88u;
    if ((uint8_t)g_edx != 0u) guest_write32(g_ecx, g_eax);
}
