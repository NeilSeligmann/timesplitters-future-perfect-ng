/* SPDX-License-Identifier: GPL-3.0-or-later */
/* T1784: synthesized-domain roots of T1479 list-002, list-003 and band 0x00380000-0x00470000.
 * Every body is a frozen draft proved by tools/replace/batch_prove with --synth-domain
 * (t1773-synth-v1), see docs/t1773-synth-group-d.md. EXACT registers, ordered guest reads and
 * writes, real guest-stack saves and reloads. */
#include "game_replace.h"
extern __thread uint32_t g_ebx, g_ebp, g_edi;
extern __thread int g_df;

static uint16_t guest_read16(uint32_t address)
{
    uint16_t value;
    memcpy(&value, game_host_ptr(address), sizeof value);
    return value;
}

static void guest_write16(uint32_t address, uint16_t value)
{
    memcpy(game_host_ptr(address), &value, sizeof value);
}

/* Original 0x0031F410: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0031F410, cdecl, 0, u32, game_get_field_0xac_of_active_record_b4_unless_shared_by_two_or_more)
{
    g_eax=guest_read32(0x762f98u);
    g_ecx=guest_read32(0x7844a8u);
    g_edx=guest_read32(g_eax+0xacu);
    g_eax=guest_read32(g_ecx+0x81a4u);
    guest_write32(g_esp-4u,g_esi);
    uint32_t matches=0u;
    if((int32_t)g_eax>0){g_ecx+=0x8254u;
    do{if(guest_read32(g_ecx)==g_edx)++matches;
    g_ecx+=0xb4u;
    --g_eax;
    }while(g_eax);
    }g_eax=(int32_t)matches>1?g_edx:0u;
    g_esi=guest_read32(g_esp-4u);

}

/* Original 0x0031FF30: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0031FF30, cdecl, 1, u32, game_mapedit_record_c4_flag_set_selected_by_active_b4_flag_2_and_mode)
{
    g_eax=game_stack_arg(0u);
    g_ecx=guest_read32(0x763070u);
    g_eax=g_eax*0xc4u+0x524f58u;
    if(g_ecx>1u){g_eax=0u;
    return;
    }uint32_t mode=g_ecx;
    g_ecx=guest_read32(0x762f98u);
    uint32_t active=g_ecx && (guest_read8(g_ecx+0xb0u)&2u);
    g_ecx=mode==0u?(active?32u:16u):(active?128u:64u);
    g_eax=(guest_read32(g_eax+0xb8u)&g_ecx)!=0u;

}

/* Original 0x00320FC0: gameplay/modes, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00320FC0, cdecl, 2, u32, game_copy_six_byte_entries_from_arg_count_size_div_8_into_static_table)
{
    g_edx=game_stack_arg(1u)>>3u;
    if(!g_edx)return;
    g_ecx=game_stack_arg(0u);
    guest_write32(g_esp-4u,g_ebx);
    g_eax=0x782e60u;
    guest_write32(g_esp-8u,g_esi);
    g_ecx-=g_eax;
    guest_write32(g_esp-12u,g_edi);
    do{uint32_t source=g_ecx+g_eax;
    uint32_t word=guest_read32(source);
    uint32_t destination=g_eax;
    guest_write32(destination,word);
    uint16_t tail=guest_read16(source+4u);
    g_eax+=6u;
    --g_edx;
    guest_write16(destination+4u,tail);
    }while(g_edx);
    g_edi=guest_read32(g_esp-12u);
    g_esi=guest_read32(g_esp-8u);
    g_ebx=guest_read32(g_esp-4u);

}

/* Original 0x00321000: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00321000, cdecl, 1, u32, game_mapedit_copy_kind_3_record_entries_from_word_table_into_array)
{
    g_eax = guest_read32(0x7844A8u);
    g_ecx = guest_read32(g_eax + 0xDD10u);
    g_edx = 0u;
    guest_write32(g_esp - 16u, g_edi);
    g_edi = 0u;
    guest_write32(g_esp - 12u, g_edx);
    if ((int32_t)g_ecx > 0) {
        guest_write32(g_esp - 20u, g_ebx);
        guest_write32(g_esp - 24u, g_ebp);
        guest_write32(g_esp - 8u, g_edx);
        guest_write32(g_esp - 28u, g_esi);
        do {
            if (guest_read32(g_edx + g_eax + 0xDD94u) == 3u) {
                g_ebx = guest_read32(g_edx + g_eax + 0xDD34u);
                g_esi = g_edx + g_eax + 0xDD34u;
                g_ecx = 0u;
                if ((int32_t)g_ebx > 0) {
                    g_eax = g_edx + g_eax + 0xDD88u;
                    guest_write32(g_esp - 4u, g_eax);
                    g_eax = game_stack_arg(0u);
                    g_edx = g_edi * 3u;
                    g_edx = g_eax + g_edx * 2u;
                    do {
                        g_eax = guest_read32(g_esp - 4u);
                        g_eax = (uint32_t)(int32_t)(int16_t)guest_read16(g_eax);
                        g_eax += g_ecx;
                        g_eax *= 3u;
                        g_eax = g_eax * 2u + 0x782E60u;
                        g_ebp = guest_read32(g_eax);
                        g_ebx = g_edx;
                        guest_write32(g_ebx, g_ebp);
                        g_eax = (g_eax & 0xFFFF0000u) | guest_read16(g_eax + 4u);
                        ++g_edi;
                        g_edx += 6u;
                        guest_write16(g_ebx + 4u, (uint16_t)g_eax);
                        g_eax = guest_read32(g_esi);
                        ++g_ecx;
                    } while ((int32_t)g_ecx < (int32_t)g_eax);
                    g_eax = guest_read32(0x7844A8u);
                    g_edx = guest_read32(g_esp - 8u);
                }
            }
            g_ecx = guest_read32(g_esp - 12u);
            g_esi = guest_read32(g_eax + 0xDD10u);
            ++g_ecx;
            g_edx += 0x8Cu;
            guest_write32(g_esp - 12u, g_ecx);
            guest_write32(g_esp - 8u, g_edx);
        } while ((int32_t)g_ecx < (int32_t)g_esi);
        g_esi = guest_read32(g_esp - 28u);
        g_ebp = guest_read32(g_esp - 24u);
        g_ebx = guest_read32(g_esp - 20u);
    }
    g_edi = guest_read32(g_esp - 16u);
}

/* Original 0x00321E60: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00321E60, cdecl, 0, u32, game_mapedit_record_counts_below_limits_10_and_0x1e)
{
 g_eax=guest_read32(0x7844A8);
 if((int32_t)guest_read32(g_eax+0x41D8)>=10) g_eax=0;
 else if((int32_t)guest_read32(g_eax+0x41E0)>=30) g_eax=0; else g_eax=1;
}

/* Original 0x00321EF0: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00321EF0, cdecl, 1, u32, game_mapedit_record_220_lacks_member_count_fields_426c_and_4234)
{
    g_eax = game_stack_arg(0u);
    g_ecx = guest_read32(0x7844A8u);
    g_eax *= 0x220u;
    g_edx = guest_read32(g_eax + g_ecx + 0x426Cu);
    g_eax += g_ecx;
    g_ecx = 1u;
    if ((int32_t)g_edx < (int32_t)g_ecx || (int32_t)guest_read32(g_eax + 0x4234u) < (int32_t)g_ecx) {
        g_eax = g_ecx;
    } else {
        g_eax = 0u;
    }
}

/* Original 0x00321F20: mapmaker/ui, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00321F20, cdecl, 1, u32, game_record_220_by_index_field_426c_is_at_least_one)
{
 g_eax=guest_read32(g_esp+4); g_edx=guest_read32(0x7844A8); g_eax*=0x220;
 g_esp-=4; guest_write32(g_esp,g_esi); g_esi=guest_read32(g_eax+g_edx+0x426C);
 g_ecx=((int32_t)g_esi>=1); g_esi=guest_read32(g_esp); g_esp+=4; g_eax=g_ecx;
}

/* Original 0x00322150: mapmaker/storage, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00322150, cdecl, 1, u32, game_find_record_b4_index_by_halfword_c_equal_arg_zero_if_absent)
{
    g_eax=guest_read32(0x7844a8u);
    g_edx=guest_read32(g_eax+0x81a4u);
    guest_write32(g_esp-4u,g_esi);
    g_ecx=0u;
    guest_write32(g_esp-8u,g_edi);
    if((int32_t)g_edx>0){uint32_t key=game_stack_arg(0u);
    g_eax+=0x81a8u;
    do{if((uint32_t)(int32_t)(int16_t)guest_read16(g_eax+12u)==key)break;
    ++g_ecx;
    g_eax+=0xb4u;
    }while((int32_t)g_ecx<(int32_t)g_edx);
    if(g_ecx==g_edx)g_eax=0u;
    }else g_eax=0u;
    g_edi=guest_read32(g_esp-8u);
    g_esi=guest_read32(g_esp-4u);

}

/* Original 0x00322190: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00322190, cdecl, 1, u32, game_mapedit_find_record_220_index_whose_member_b4_has_word_c_equal_arg)
{
    guest_write32(g_esp - 4u, g_ebx);
    g_ebx = guest_read32(0x7844A8u);
    guest_write32(g_esp - 8u, g_ebp);
    g_ebp = guest_read32(g_ebx + 0x81A4u);
    guest_write32(g_esp - 12u, g_esi);
    g_esi = 0u;
    guest_write32(g_esp - 16u, g_edi);
    int found = 0;
    if ((int32_t)g_ebp > 0) {
        g_edi = g_ebx + 0x8250u;
        do {
            g_eax = (uint32_t)(int32_t)(int16_t)guest_read16(g_edi - 0x9Cu);
            if (g_eax == game_stack_arg(0u) && guest_read32(g_edi) == 1u) {
                g_edx = guest_read32(g_ebx + 0x41E0u);
                g_eax = 0u;
                if ((int32_t)g_edx > 0) {
                    g_ecx = g_ebx + 0x4238u;
                    do {
                        if (g_esi == guest_read32(g_ecx + 0x38u) || g_esi == guest_read32(g_ecx)) {
                            found = 1;
                            break;
                        }
                        ++g_eax;
                        g_ecx += 0x220u;
                    } while ((int32_t)g_eax < (int32_t)g_edx);
                    if (found) {
                        break;
                    }
                }
            }
            ++g_esi;
            g_edi += 0xB4u;
        } while ((int32_t)g_esi < (int32_t)g_ebp);
    }
    if (!found) {
        g_eax = 0xFFFFFFFFu;
    }
    g_edi = guest_read32(g_esp - 16u);
    g_esi = guest_read32(g_esp - 12u);
    g_ebp = guest_read32(g_esp - 8u);
    g_ebx = guest_read32(g_esp - 4u);
}

/* Original 0x00322200: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00322200, cdecl, 0, u32, game_current_selection_id_from_fields_2f1c_2f78_2f38_2f34)
{
    g_ecx=guest_read32(0x7844a8u);
    g_eax=guest_read32(g_ecx+0x2f1cu);
    if(g_eax){g_eax=guest_read32(g_eax+4u);
    if((int32_t)g_eax<0)g_eax=0xffffffffu;
    return;
    }g_eax=guest_read32(g_ecx+0x2f78u);
    if((int32_t)g_eax>=0){g_eax*=3u;
    g_eax=(uint32_t)(int32_t)(int16_t)guest_read16(g_ecx+g_eax*4u+0x307au);
    if((int32_t)g_eax<0)g_eax=0xffffffffu;
    return;
    }g_eax=guest_read32(g_ecx+0x2f38u);
    if((int32_t)g_eax<0){g_eax=0xffffffffu;
    return;
    }g_edx=guest_read32(g_ecx+0x2f34u);
    if((int32_t)g_edx<0)g_eax=0xffffffffu;

}

/* Original 0x003226E0: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(003226E0, cdecl, 0, u32, game_has_record_b4_with_kind_0_to_2_not_flag_200_and_byte_8258_bit0)
{
    g_ecx = guest_read32(0x7844A8u);
    guest_write32(g_esp - 4u, g_esi);
    guest_write32(g_esp - 8u, g_edi);
    g_edi = guest_read32(g_ecx + 0x81A4u);
    g_eax = 0u;
    g_esi = 0u;
    if ((int32_t)g_edi > 0) {
        g_ecx += 0x8258u;
        do {
            int check = 0;
            g_edx = guest_read32(g_ecx - 0xB0u);
            if (g_edx == 0u) {
                check = 1;
            } else if ((int32_t)g_edx > 0 && (int32_t)g_edx <= 2) {
                g_edx = guest_read32(g_ecx - 0xACu);
                g_edx *= 0x4Cu;
                if ((guest_read32(g_edx + 0x52E0D8u) & 0x200u) == 0u) {
                    check = 1;
                }
            }
            if (check && (guest_read8(g_ecx) & 1u) != 0u) {
                g_eax = 1u;
                break;
            }
            ++g_esi;
            g_ecx += 0xB4u;
        } while ((int32_t)g_esi < (int32_t)g_edi);
    }
    g_edi = guest_read32(g_esp - 8u);
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x0032D1A0: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0032D1A0, cdecl, 4, u32, game_mapedit_set_view_rect_with_aspect_fit_and_reset_steps)
{
    g_ecx = guest_read32(0x778D34u);
    g_eax = game_stack_arg(0u);
    g_edx = game_stack_arg(1u);
    guest_write32(g_ecx + 0x14u, g_eax);
    g_ecx = guest_read32(0x778D34u);
    guest_write32(g_ecx + 0x18u, g_edx);
    g_ecx = guest_read32(0x778D34u);
    guest_write32(g_esp - 4u, g_ebx);
    guest_write32(g_esp - 8u, g_esi);
    g_esi = game_stack_arg(2u);
    guest_write32(g_ecx + 0x1Cu, g_esi);
    g_ecx = game_stack_arg(3u);
    guest_write32(g_esp - 12u, g_edi);
    g_edi = guest_read32(0x778D34u);
    guest_write32(g_edi + 0x20u, g_ecx);
    g_edi = guest_read32(0x778D34u);
    guest_write32(g_edi + 4u, g_eax);
    g_eax = guest_read32(0x778D34u);
    guest_write32(g_eax + 8u, g_edx);
    g_edi = guest_read32(0x4E8CF4u);
    g_ebx = guest_read32(0x4E8CF0u);
    /* imul, cdq, idiv: a zero divisor or INT_MIN / -1 faults like the original. */
    g_eax = (uint32_t)((int32_t)(g_edi * g_esi) / (int32_t)g_ebx);
    g_edx = 0u;
    if ((int32_t)g_eax > (int32_t)g_ecx) {
        g_ecx = guest_read32(0x778D34u);
        guest_write32(g_ecx + 0xCu, g_esi);
        g_eax = guest_read32(0x4E8CF4u);
        g_eax *= g_esi;
        g_eax = (uint32_t)((int32_t)g_eax / (int32_t)guest_read32(0x4E8CF0u));
        g_edx = guest_read32(0x778D34u);
        g_edi = guest_read32(g_esp - 12u);
        g_esi = guest_read32(g_esp - 8u);
        g_ebx = guest_read32(g_esp - 4u);
        guest_write32(g_edx + 0x10u, g_eax);
        g_eax = guest_read32(0x778D34u);
        guest_write32(g_eax + 0x34u, 0u);
        return;
    }
    g_eax = g_ebx * g_ecx;
    g_eax = (uint32_t)((int32_t)g_eax / (int32_t)g_edi);
    g_edx = guest_read32(0x778D34u);
    g_edi = guest_read32(g_esp - 12u);
    g_esi = guest_read32(g_esp - 8u);
    g_ebx = guest_read32(g_esp - 4u);
    guest_write32(g_edx + 0xCu, g_eax);
    g_eax = guest_read32(0x778D34u);
    guest_write32(g_eax + 0x10u, g_ecx);
    g_ecx = guest_read32(0x778D34u);
    guest_write32(g_ecx + 0x34u, 0u);
}

/* Original 0x00331970: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00331970, cdecl, 1, u32, game_get_context_descriptor_flag_bit_1_of_byte_0x12_by_index)
{
g_eax = game_stack_arg(0u) * 320u; g_eax = (guest_read8(g_eax + 0x529522u) >> 1u) & 1u;
}

/* Original 0x00331DD0: mapmaker/editor, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00331DD0, cdecl, 1, u32, game_count_context_entries_with_bit_6_set_by_index)
{
    g_edx=game_stack_arg(0u);
    g_ecx=guest_read32(0x789440u+g_edx*4u);
    g_edx=guest_read32(0x785000u+g_edx*4u);
    g_edx=guest_read32(g_edx+4u);
    g_eax=0u;
    g_ecx+=12u;
    if((int32_t)g_edx>0){g_ecx+=4u;
    guest_write32(g_esp-4u,g_esi);
    do{g_eax+=(guest_read32(g_ecx)>>6u)&1u;
    g_ecx+=32u;
    --g_edx;
    }while(g_edx);
    g_esi=guest_read32(g_esp-4u);
    }
}

/* Original 0x003383A0: data/missions, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(003383A0, cdecl, 1, u32, game_script_find_entry_28_by_key_dword_4)
{
    g_ecx = guest_read32(0x765E44u);
    g_edx = guest_read32(g_ecx + 0x80u);
    guest_write32(g_esp - 4u, g_esi);
    g_eax = 0u;
    guest_write32(g_esp - 8u, g_edi);
    int found = 0;
    if ((int32_t)g_edx > 0) {
        g_esi = guest_read32(g_ecx + 0x84u);
        g_edi = game_stack_arg(0u);
        g_ecx = g_esi + 4u;
        do {
            if (guest_read32(g_ecx) == g_edi) {
                found = 1;
                break;
            }
            ++g_eax;
            g_ecx += 0x28u;
        } while ((int32_t)g_eax < (int32_t)g_edx);
    }
    if (found) {
        g_eax = g_eax * 5u;
        g_edi = guest_read32(g_esp - 8u);
        g_eax = g_esi + g_eax * 8u + 4u;
        g_esi = guest_read32(g_esp - 4u);
    } else {
        g_edi = guest_read32(g_esp - 8u);
        g_esi = guest_read32(g_esp - 4u);
        g_eax = 0u;
    }
}

/* Original 0x003397F0: data/missions, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(003397F0, cdecl, 1, u32, game_script_context_record_58_address_by_index)
{
    g_eax = guest_read32(g_esp + 4); g_ecx = guest_read32(0x765E44);
    g_eax *= 0x58; g_eax += guest_read32(g_ecx + 0xB0);
}

/* Original 0x0033E620: data/missions, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0033E620, cdecl, 1, u32, game_script_action_entry_kind_2_ready_flag_4_exists_for_arg)
{
    g_eax = guest_read32(0x765E44u);
    g_edx = guest_read32(g_eax + 0x88u);
    guest_write32(g_esp - 4u, g_esi);
    g_esi = 0u;
    g_ecx = 0u;
    guest_write32(g_esp - 8u, g_edi);
    if ((int32_t)g_edx > 0) {
        g_eax = guest_read32(g_eax + 0x8Cu);
        g_edi = game_stack_arg(0u);
        g_eax += 4u;
        do {
            if (guest_read32(g_eax + 0x10u) == g_edi && guest_read32(g_eax) == 2u &&
                guest_read32(g_eax + 0xCu) == 0x339770u) {
                if ((guest_read8(g_eax + 8u) & 4u) != 0u) {
                    g_edi = guest_read32(g_esp - 8u);
                    g_eax = 1u;
                    g_esi = guest_read32(g_esp - 4u);
                    return;
                }
                ++g_esi;
            }
            ++g_ecx;
            g_eax += 0x24u;
        } while ((int32_t)g_ecx < (int32_t)g_edx);
    }
    g_eax = (g_esi == 0u) ? 1u : 0u;
    g_edi = guest_read32(g_esp - 8u);
    g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x0033EC80: data/missions, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0033EC80, cdecl, 1, u32, game_script_context_array_a0_record_16_flag_bit3_is_clear)
{
    g_eax = game_stack_arg(0u);
    g_ecx = guest_read32(0x765E44u);
    g_edx = guest_read32(g_ecx + 0xA0u);
    g_eax <<= 4;
    g_eax = guest_read32(g_edx + g_eax + 8u);
    g_eax >>= 3;
    g_eax = ~g_eax;
    g_eax &= 1u;
}

/* Original 0x0033ECA0: data/missions, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0033ECA0, cdecl, 1, u32, game_script_context_array_98_record_16_flag_bit3_is_clear)
{
    g_eax = game_stack_arg(0u);
    g_ecx = guest_read32(0x765E44u);
    g_edx = guest_read32(g_ecx + 0x98u);
    g_eax <<= 4;
    g_eax = guest_read32(g_edx + g_eax + 8u);
    g_eax >>= 3;
    g_eax = ~g_eax;
    g_eax &= 1u;
}

/* Original 0x003530F0: network/session, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT_INPUTS(003530F0, cdecl, 1, u32, ecx, game_net_packet_queue_114_clear_entries_for_owner_id)
{
    guest_write32(g_esp - 4u, g_ecx);
    g_edx = guest_read32(0x76ADF0u);
    guest_write32(g_esp - 8u, g_ebx);
    g_edx += 0x110u;
    guest_write32(g_esp - 12u, g_edi);
    guest_write32(g_esp - 4u, 0x800u);
    g_ebx = 0u;
    do {
        g_eax = game_stack_arg(0u);
        if (guest_read32(g_edx - 0x110u) == g_eax) {
            guest_write32(g_edx, guest_read32(g_edx) & 0xFFFFFFFEu);
            guest_write32(g_edx - 8u, g_ebx);
            guest_write8(g_edx - 4u, (uint8_t)g_ebx);
            guest_write16(g_edx - 0x10Cu, 0xFFFFu);
            guest_write32(g_edx - 0x110u, 0xFFFFFFFFu);
            g_eax = 0u;
            g_ecx = 0x3Fu;
            g_edi = g_edx - 0x10Au;
            while (g_ecx != 0u) {
                guest_write32(g_edi, g_eax);
                g_edi += g_df ? 0xFFFFFFFCu : 4u;
                --g_ecx;
            }
            guest_write16(g_edi, (uint16_t)g_eax);
            g_edi += g_df ? 0xFFFFFFFEu : 2u;
            guest_write8(g_edi, (uint8_t)g_eax);
            g_edi += g_df ? 0xFFFFFFFFu : 1u;
        }
        g_eax = guest_read32(g_esp - 4u);
        g_edx += 0x114u;
        --g_eax;
        guest_write32(g_esp - 4u, g_eax);
    } while (g_eax != 0u);
    g_edi = guest_read32(g_esp - 12u);
    g_ebx = guest_read32(g_esp - 8u);
    g_ecx = guest_read32(g_esp - 4u);
}

/* Original 0x0035DEC0: network/session, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035DEC0, cdecl, 1, u32, game_net_clear_peer_mask_entry_by_index_or_all)
{
g_eax=game_stack_arg(0u);if(g_eax==0xffffffffu){g_eax=0u;for(uint32_t offset=0u;offset<64u;offset+=4u)guest_write32(0x76b1c8u+offset,0u);}else guest_write32(g_eax*4u+0x76b1c8u,0u);
}

/* Original 0x0035E1B0: network/session, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035E1B0, cdecl, 1, u32, game_net_get_roster_record_by_index_when_in_use)
{
    g_eax = guest_read32(g_esp + 4u) * 0x78u;
    const uint8_t flag = guest_read8(g_eax + 0x76b2c8u);
    g_ecx = (g_ecx & 0xffffff00u) | flag;
    if (flag != 0u) {
        g_eax += 0x76b2b0u;
    } else {
        g_eax = 0u;
    }
}

/* Original 0x0035E1D0: network/session, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035E1D0, cdecl, 2, u32, game_net_store_session_slot_record_18_by_index_and_mark_present)
{
g_eax=game_stack_arg(0u);guest_write32(g_esp-4u,g_esi);g_ecx=g_eax*3u;g_edx=g_ecx*8u+0x76ba30u;g_ecx=game_stack_arg(1u);for(uint32_t offset=0u;offset<20u;offset+=4u)guest_write32(g_edx+offset,guest_read32(g_ecx+offset));g_ecx=guest_read32(g_ecx+20u);guest_write32(g_edx+20u,g_ecx);guest_write32(g_eax*4u+0x76b210u,1u);
g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x0035E220: network/session, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035E220, cdecl, 1, u32, game_net_get_session_slot_record_18_by_index_when_present)
{
    g_eax = guest_read32(g_esp + 4u);
    g_ecx = guest_read32(g_eax * 4u + 0x76b210u);
    if (g_ecx != 0u) {
        g_eax = g_eax * 24u + 0x76ba30u; /* lea eax,[eax+eax*2]; lea eax,[eax*8+0x76BA30] */
    } else {
        g_eax = 0u;
    }
}

/* Original 0x0035E250: network/session, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035E250, cdecl, 2, u32, game_net_store_local_user_record_18_by_index_and_set_stored_flag)
{
    const uint32_t source = guest_read32(g_esp + 8u);
    g_eax = guest_read32(g_esp + 4u);
    g_ecx = g_eax * 24u + 0x76b250u;
    for (uint32_t offset = 0u; offset < 0x14u; offset += 4u) {
        g_eax = guest_read32(source + offset);
        guest_write32(g_ecx + offset, g_eax);
    }
    g_edx = guest_read32(source + 0x14u);
    guest_write32(g_ecx + 0x14u, g_edx);
    guest_write32(0x76bbb0u, 1u);
}

/* Original 0x0035EF20: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035EF20, cdecl, 1, u32, game_shared_online_begin_collecting_results_storing_source_tag)
{
g_eax=guest_read32(0x76bd7cu);if(g_eax){g_eax=0u;return;}g_eax=guest_read32(0x76bd6cu);if(g_eax){g_eax=0u;return;}g_ecx=game_stack_arg(0u);g_edx=guest_read32(0x76bd74u);g_eax=1u;guest_write32(0x76bd7cu,g_eax);guest_write32(0x76bd6cu,g_eax);guest_write32(0x76bd80u,0u);guest_write32(0x76bd84u,g_ecx);guest_write32(g_edx,0u);
}

/* Original 0x0035EF70: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035EF70, cdecl, 1, u32, game_shared_online_append_result_record_0x88_six_dwords_with_source_tag)
{
g_ecx=guest_read32(0x76bd80u);g_edx=guest_read32(0x76bd74u);g_eax=g_ecx*0x88u+g_edx;g_edx=guest_read32(0x76bd84u);guest_write32(g_eax+0x74u,g_edx);g_edx=game_stack_arg(0u);guest_write32(g_esp-4u,g_esi);g_eax+=0x38u;for(uint32_t offset=0u;offset<20u;offset+=4u)guest_write32(g_eax+offset,guest_read32(g_edx+offset));g_edx=guest_read32(g_edx+20u);++g_ecx;guest_write32(g_eax+20u,g_edx);guest_write32(0x76bd80u,g_ecx);
g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x0035EFD0: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035EFD0, cdecl, 1, u32, game_shared_online_append_result_record_0x88_two_dwords_at_50_54)
{
g_ecx=guest_read32(0x76bd80u);g_edx=guest_read32(0x76bd74u);g_eax=g_ecx*0x88u+g_edx;g_edx=guest_read32(0x76bd84u);guest_write32(g_eax+0x74u,g_edx);g_edx=game_stack_arg(0u);guest_write32(g_esp-4u,g_esi);guest_write32(g_eax+0x50u,guest_read32(g_edx));g_edx=guest_read32(g_edx+4u);++g_ecx;guest_write32(g_eax+0x54u,g_edx);guest_write32(0x76bd80u,g_ecx);
g_esi = guest_read32(g_esp - 4u);
}

/* Original 0x0035F1A0: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0035F1A0, cdecl, 0, u32, game_shared_online_clear_all_search_entries_and_session_counters)
{
    g_eax=guest_read32(0x76bd78u);
    g_ecx=0u;
    guest_write8(0x76bd70u,0u);
    guest_write8(0x76bd68u,0u);
    guest_write32(0x76bd6cu,0u);
    g_eax-=8u;
    g_edx=20u;
    do{guest_write8(g_eax+1u,0u);
    guest_write8(g_eax,0u);
    guest_write8(g_eax+3u,0u);
    g_eax+=0xaa8u;
    --g_edx;
    }while(g_edx);

}

/* Original 0x0036E3A0: data/missions, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(0036E3A0, cdecl, 2, u32, game_terminal_bring_widget_to_front_reorder_layers_and_set_focus)
{
    g_ecx = game_stack_arg(1u);
    g_eax = guest_read32(0x76C274u);
    g_ecx *= 0x4Cu;
    g_edx = guest_read32(g_eax + 0x10u);
    guest_write32(g_esp - 4u, g_ebx);
    guest_write32(g_esp - 8u, g_esi);
    g_esi = guest_read32(g_eax + 0xCu);
    g_ebx = guest_read32(g_ecx + g_esi + 0x18u);
    guest_write32(g_esp - 12u, g_edi);
    g_ecx += g_esi;
    g_edi = 0u;
    if ((int32_t)g_edx > 0) {
        g_edx = 0u;
        guest_write32(g_esp - 16u, g_ebp);
        do {
            g_esi = guest_read32(g_eax + 0xCu);
            if ((int32_t)guest_read32(g_edx + g_esi + 0x18u) > (int32_t)g_ebx) {
                g_ebp = guest_read32(g_edx + g_esi + 0x18u);
                g_esi = g_edx + g_esi + 0x18u;
                --g_ebp;
                guest_write32(g_esi, g_ebp);
            }
            g_esi = guest_read32(g_eax + 0x10u);
            ++g_edi;
            g_edx += 0x4Cu;
        } while ((int32_t)g_edi < (int32_t)g_esi);
        g_ebp = guest_read32(g_esp - 16u);
    }
    g_eax = guest_read32(g_eax + 0x10u);
    g_edi = guest_read32(g_esp - 12u);
    --g_eax;
    g_esi = guest_read32(g_esp - 8u);
    guest_write32(g_ecx + 0x18u, g_eax);
    guest_write32(0x76C290u, g_ecx);
    g_ebx = guest_read32(g_esp - 4u);
}

/* Original 0x00379730: unknown, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(00379730, cdecl, 9, u32, game_table_74c36c_find_entry_by_key_return_two_words_and_base_plus_prefix_sum_times_24)
{
    guest_write32(g_esp - 12u, g_esi);
    guest_write32(g_esp - 16u, g_edi);
    g_edi = guest_read32(0x74C36Cu);
    g_esi = guest_read32(g_edi + 0x58u);
    g_edx = 0u;
    g_eax = 0u;
    if ((int32_t)g_esi > 0) {
        g_ecx = guest_read32(g_edi + 0x54u);
        guest_write32(g_esp - 20u, g_ebx);
        g_ebx = game_stack_arg(0u);
        guest_write32(g_esp - 24u, g_ebp);
        do {
            if (guest_read32(g_ecx) == g_ebx) {
                g_ecx = guest_read32(g_edi + 0x54u);
                g_eax = g_ecx + g_eax * 8u;
                g_ecx = (g_ecx & 0xFFFF0000u) | guest_read16(g_eax + 4u);
                g_eax = (g_eax & 0xFFFF0000u) | guest_read16(g_eax + 6u);
                g_ebp = guest_read32(g_esp - 24u);
                guest_write16(g_esp - 8u, (uint16_t)g_ecx);
                g_ecx = g_edx * 3u;
                g_edx = guest_read32(g_edi + 0x5Cu);
                g_ebx = guest_read32(g_esp - 20u);
                guest_write16(g_esp - 6u, (uint16_t)g_eax);
                g_eax = guest_read32(g_esp - 8u);
                g_edi = guest_read32(g_esp - 16u);
                g_edx = g_edx + g_ecx * 8u;
                g_esi = guest_read32(g_esp - 12u);
                return;
            }
            g_ebp = guest_read16(g_ecx + 4u);
            g_edx += g_ebp;
            ++g_eax;
            g_ecx += 8u;
        } while ((int32_t)g_eax < (int32_t)g_esi);
        g_edx = guest_read32(g_esp - 4u);
        g_eax = guest_read32(g_esp - 8u);
        g_ebp = guest_read32(g_esp - 24u);
        g_ebx = guest_read32(g_esp - 20u);
        g_edi = guest_read32(g_esp - 16u);
        g_esi = guest_read32(g_esp - 12u);
        return;
    }
    g_edx = guest_read32(g_esp - 4u);
    g_eax = guest_read32(g_esp - 8u);
    g_edi = guest_read32(g_esp - 16u);
    g_esi = guest_read32(g_esp - 12u);
}

/* Original 0x003B38E0: network/live, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(003B38E0, stdcall, 2, u32, game_fesl_string_array_get_string_pointer_at_index_8_byte_entries_faulting_when_out_of_range)
{
    g_ecx = game_stack_arg(0u);
    g_eax = game_stack_arg(1u);
    if ((int32_t)g_eax < (int32_t)guest_read32(g_ecx + 4u)) {
        g_ecx = guest_read32(g_ecx);
        g_eax = guest_read32(g_ecx + g_eax * 8u);
        return;
    }
    g_eax = 0u;
    g_eax = guest_read32(g_eax); /* deliberate null dereference of the original */
}

/* Original 0x003C7360: network/live, T1784 synthesized-domain draft; EXACT registers, ordered guest reads/writes, real guest-stack saves. */
GAME_REPLACE_EXACT(003C7360, cdecl, 5, u32, game_fesl_rsa_public_key_store_modulus_length_and_exponent_length_into_context_fields_0x400_0x488)
{
    g_eax = game_stack_arg(0u);
    g_ecx = game_stack_arg(2u);
    g_edx = game_stack_arg(4u);
    guest_write32(g_eax + 0x400u, g_ecx);
    guest_write32(g_esp - 4u, g_ebx);
    g_ebx = g_ecx;
    guest_write32(g_esp - 8u, g_esi);
    g_esi = game_stack_arg(1u);
    g_ecx >>= 2;
    guest_write32(g_esp - 12u, g_edi);
    guest_write32(g_eax + 0x488u, g_edx);
    g_edi = g_eax + 0x404u;
    while (g_ecx != 0u) {
        guest_write32(g_edi, guest_read32(g_esi));
        g_esi += g_df ? 0xFFFFFFFCu : 4u;
        g_edi += g_df ? 0xFFFFFFFCu : 4u;
        --g_ecx;
    }
    g_ecx = g_ebx & 3u;
    while (g_ecx != 0u) {
        guest_write8(g_edi, guest_read8(g_esi));
        g_esi += g_df ? 0xFFFFFFFFu : 1u;
        g_edi += g_df ? 0xFFFFFFFFu : 1u;
        --g_ecx;
    }
    g_esi = game_stack_arg(3u);
    g_ecx = g_edx;
    g_edi = g_eax + 0x48Cu;
    g_eax = g_ecx;
    g_ecx >>= 2;
    while (g_ecx != 0u) {
        guest_write32(g_edi, guest_read32(g_esi));
        g_esi += g_df ? 0xFFFFFFFCu : 4u;
        g_edi += g_df ? 0xFFFFFFFCu : 4u;
        --g_ecx;
    }
    g_ecx = g_eax & 3u;
    while (g_ecx != 0u) {
        guest_write8(g_edi, guest_read8(g_esi));
        g_esi += g_df ? 0xFFFFFFFFu : 1u;
        g_edi += g_df ? 0xFFFFFFFFu : 1u;
        --g_ecx;
    }
    g_edi = guest_read32(g_esp - 12u);
    g_esi = guest_read32(g_esp - 8u);
    g_ebx = guest_read32(g_esp - 4u);
}
