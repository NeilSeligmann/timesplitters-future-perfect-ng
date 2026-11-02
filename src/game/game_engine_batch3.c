/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1787 batch3 register-exact admissions; names INFERRED. Frozen receipts in docs/. */
#include "game_replace.h"
extern __thread uint32_t g_ebx,g_edi,g_ebp;
GAME_REPLACE_EXACT(00071970, cdecl, 3, u32, game_object_nth_matching_node_batch3)
{ g_eax=guest_read32(g_esp+4);g_eax=guest_read32(g_eax+0x178);if(g_eax){g_edx=guest_read32(g_esp+8);g_ecx=guest_read32(g_esp+12);do{if(guest_read32(g_eax+0x14)==g_edx){if(!g_ecx)return;--g_ecx;}g_eax=guest_read32(g_eax+4);}while(g_eax);}g_eax=0; }
GAME_REPLACE_EXACT(00074AB0, cdecl, 3, u32, game_object_matching_node_child_index_batch3)
{ g_eax=guest_read32(g_esp+4);g_eax=guest_read32(g_eax+0x178);if(g_eax){g_ecx=guest_read32(g_esp+8);while(guest_read32(g_eax+0x14)!=g_ecx){g_eax=guest_read32(g_eax+4);if(!g_eax)break;}}g_edx=g_eax;g_eax=guest_read32(g_edx+0xD8);g_ecx=0;if(!g_eax)return;g_esp-=4;guest_write32(g_esp,g_esi);g_esi=guest_read32(g_esp+16);while(guest_read32(g_eax+8)!=g_esi){g_eax=guest_read32(g_eax+16);++g_ecx;if(!g_eax)break;}if(g_eax){guest_write32(g_edx+0xCC,g_esi);guest_write32(g_edx+0xC8,g_ecx);}g_esi=guest_read32(g_esp);g_esp+=4; }
GAME_REPLACE_EXACT(0008E3E0, cdecl, 2, u32, game_render_relocate_flagged_records_batch3)
{ g_ecx=guest_read32(g_esp+4);if(guest_read32(g_ecx)==UINT32_MAX)return;g_esp-=4;guest_write32(g_esp,g_esi);g_esi=guest_read32(g_esp+12);g_edx=g_ecx;do{g_eax=guest_read32(g_ecx);if(g_eax!=0x40000000&&(guest_read32(g_ecx+12)&0x10000000)){if(g_eax)g_eax+=g_esi;guest_write32(g_ecx,g_eax);}g_eax=guest_read32(g_edx+16);g_edx+=16;g_ecx=g_edx;}while(g_eax!=UINT32_MAX);g_esi=guest_read32(g_esp);g_esp+=4; }
GAME_REPLACE_EXACT(000B0770, cdecl, 1, u32, game_object_current_or_previous_reference_batch3)
{ g_eax=guest_read32(0x7DE318);if(g_eax){g_ecx=guest_read32(g_esp+4);g_ecx=guest_read32(g_ecx+0x90);if(g_ecx){if(g_ecx==g_eax){g_eax=1;return;}g_eax-=guest_read32(0x4E7944);if(g_ecx==g_eax){g_eax=1;return;}}}g_eax=0; }
GAME_REPLACE_EXACT(000B3CA0, cdecl, 1, u32, game_object_optional_record_lookup_batch3)
{ g_eax=guest_read32(g_esp+4);g_eax=guest_read32(g_eax+0x7C);if(g_eax&&(guest_read8(g_eax+8)&0x40)){g_eax=guest_read32(g_eax+12);if((int32_t)g_eax>=0&&(int32_t)g_eax<25){g_eax=g_eax*0xB4+0x4D98B8;return;}}g_eax=0; }
GAME_REPLACE_EXACT(000D76A0, cdecl, 2, u32, game_audio_table_special_fallback_batch3)
{ g_ecx=guest_read32(g_esp+4);g_eax=g_ecx*3;g_eax<<=4;g_eax=guest_read32(g_eax+0x53A3CC);if(!g_eax&&g_ecx==0x24){g_eax=guest_read32(g_esp+8);g_eax=g_eax?0x53A2C8:0x539B08;} }
GAME_REPLACE_EXACT(0015C120, cdecl, 1, u32, game_object_clear_matching_owner_batch3)
{ g_eax=guest_read32(0x74C37C);g_edx=0;g_ecx=0;if((int32_t)g_eax<=0)return;g_esp-=4;guest_write32(g_esp,g_ebx);g_esp-=4;guest_write32(g_esp,g_edi);g_edi=guest_read32(g_esp+12);do{g_eax=guest_read32(g_ecx*4+0x7A3580);if(g_eax&&guest_read32(g_eax+4)==g_edi){g_ebx=guest_read32(g_eax+0x2C)&0xFFFFFF7F;guest_write32(g_eax+4,0);guest_write32(g_eax+0x2C,g_ebx);}g_eax=guest_read32(0x74C37C);++g_ecx;}while((int32_t)g_ecx<(int32_t)g_eax);g_edi=guest_read32(g_esp);g_esp+=4;g_ebx=guest_read32(g_esp);g_esp+=4; }
GAME_REPLACE_EXACT(00160F50, cdecl, 1, u32, game_object_vehicle_tag_test_batch3)
{ g_eax=guest_read32(g_esp+4);uint32_t type=guest_read32(g_eax+0x20);g_ecx=guest_read32(g_eax+0x7C);if(type==8){g_eax=guest_read32(g_ecx+0xBF0);g_eax=guest_read32(g_eax+8);g_eax=(g_eax==0x45434956||g_eax==0x4F434956);}else g_eax=0; }
GAME_REPLACE_EXACT(001766E0, cdecl, 2, u32, game_object_slot_handle_invalid_batch3)
{ g_eax=guest_read32(g_esp+8);g_ecx=guest_read32(g_esp+4);g_eax*=0x34;g_edx=guest_read32(g_ecx+0x7C);g_ecx=guest_read32(g_edx+g_eax+0x70);g_edx+=g_eax+0x70;if(g_ecx){g_eax=(g_ecx>>22)*0x2E8;g_eax+=guest_read32(0x72F5E8);if(guest_read32(g_eax+0x2C)==g_ecx){g_eax=0;return;}guest_write32(g_edx,0);}g_eax=1; }
GAME_REPLACE_EXACT(00176A40, cdecl, 3, u32, game_object_slot_handle_set_batch3)
{ g_ecx=guest_read32(g_esp+12);g_eax=guest_read32(g_esp+4);g_ecx*=0x34;g_eax=guest_read32(g_eax+0x7C);g_edx=g_ecx+g_eax+0x70;g_ecx=guest_read32(g_edx);if(g_ecx){g_eax=(g_ecx>>22)*0x2E8;g_eax+=guest_read32(0x72F5E8);if(guest_read32(g_eax+0x2C)==g_ecx)return;guest_write32(g_edx,0);}g_eax=guest_read32(g_esp+8);g_ecx=guest_read32(g_eax+0x2C);guest_write32(g_edx,g_ecx);guest_write8(g_eax+0x32,2); }
GAME_REPLACE_EXACT(00188920, cdecl, 1, u32, game_object_extension_four_categories_batch3)
{ g_eax=guest_read32(g_esp+4);if(g_eax){g_eax=guest_read32(g_eax+0x7C);g_eax=guest_read32(g_eax+0x34);g_eax=(g_eax==2||g_eax==5||g_eax==7||g_eax==3);} }
GAME_REPLACE_EXACT(00240020, cdecl, 1, u32, game_object_named_handle_resolve_batch3)
{ g_ecx=guest_read32(g_esp+4);g_eax=0;while(guest_read32(g_eax*8+0x75CE68)!=g_ecx){++g_eax;if((int32_t)g_eax>=150){g_eax=0;return;}}g_eax=guest_read32(g_eax*8+0x75CE6C);g_edx=guest_read32(g_eax);if(!g_edx){g_eax=0;return;}g_ecx=(g_edx>>22)*0x2E8;g_ecx+=guest_read32(0x72F5E8);if(guest_read32(g_ecx+0x2C)==g_edx)g_eax=g_ecx;else{guest_write32(g_eax,0);g_eax=0;} }
