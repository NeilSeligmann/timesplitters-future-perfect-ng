/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "game_replace.h"
extern __thread uint32_t g_ebx,g_edi,g_ebp;
typedef union EngineB5Xmm { uint32_t u[4]; uint64_t q[2]; float f[4]; } EngineB5Xmm;
__thread EngineB5Xmm g_xmm0 __attribute__((weak,aligned(8)));
__thread EngineB5Xmm g_xmm1 __attribute__((weak,aligned(8)));
static inline void b5_load(EngineB5Xmm *x,uint32_t a){x->u[0]=guest_read32(a);x->u[1]=x->u[2]=x->u[3]=0;}
static inline float b5_float(uint32_t b){float f;memcpy(&f,&b,4);return f;}
static inline int b5_below(float a,float b){int t;__asm__ volatile("comiss %2, %1":"=@ccb"(t):"x"(a),"xm"(b));return t;}
static inline int b5_above(float a,float b){int t;__asm__ volatile("comiss %2, %1":"=@cca"(t):"x"(a),"xm"(b));return t;}
GAME_REPLACE_EXACT(000194A0, cdecl, 2, u32, game_effect_append_pair_batch5)
{ g_eax=guest_read32(0x55F974);b5_load(&g_xmm0,g_esp+4);guest_write32(g_eax,g_xmm0.u[0]);g_eax=guest_read32(0x55F974);b5_load(&g_xmm0,g_esp+8);g_eax+=4;guest_write32(0x55F974,g_eax);guest_write32(g_eax,g_xmm0.u[0]);guest_write32(0x55F974,guest_read32(0x55F974)+4); }
GAME_REPLACE_EXACT(000746A0, cdecl, 1, u32, game_object_clear_three_batch5)
{ g_eax=guest_read32(g_esp+4);g_xmm0.q[0]=g_xmm0.q[1]=0;guest_write32(g_eax+0x84,0);guest_write32(g_eax+0xC0,0);guest_write32(g_eax+0x88,0); }
GAME_REPLACE_EXACT(000AEE60, cdecl, 1, u32, game_extension_storea8_batch5)
{ g_eax=guest_read32(g_esp+4);g_ecx=guest_read32(g_eax+0x7C);b5_load(&g_xmm0,0x475D18);guest_write32(g_ecx+0xA8,g_xmm0.u[0]); }
GAME_REPLACE_EXACT(000B3C60, cdecl, 1, u32, game_extension_count_float_predicate_batch5)
{ g_eax=guest_read32(g_esp+4);g_ecx=guest_read32(g_eax+0x7C);g_eax=guest_read32(g_ecx+0x6C);g_esp-=4;guest_write32(g_esp,g_esi);g_esi=guest_read32(g_eax+0xF0);g_edx=10;if((int32_t)g_esi<10||(int32_t)guest_read32(g_eax+0xF4)<10){g_xmm0.q[0]=g_xmm0.q[1]=0;if(!b5_below(g_xmm0.f[0],b5_float(guest_read32(g_ecx+0x1C4)))){g_eax=1;g_esi=guest_read32(g_esp);g_esp+=4;return;}}g_eax=0;g_esi=guest_read32(g_esp);g_esp+=4; }
GAME_REPLACE_EXACT(000E10E0, cdecl, 2, u32, game_object_store9c_batch5)
{ g_eax=guest_read32(g_esp+4);if(g_eax){b5_load(&g_xmm0,g_esp+8);guest_write32(g_eax+0x9C,g_xmm0.u[0]);} }
GAME_REPLACE_EXACT(000FF3D0, cdecl, 2, u32, game_effect_store38_batch5)
{ g_eax=guest_read32(g_esp+4);if(g_eax){b5_load(&g_xmm0,g_esp+8);guest_write32(g_eax+0x38,g_xmm0.u[0]);} }
GAME_REPLACE_EXACT(00111560, cdecl, 1, u32, game_effect_store70_batch5)
{ g_eax=guest_read32(0x73BD64);if(g_eax){b5_load(&g_xmm0,g_esp+4);guest_write32(g_eax+0x70,g_xmm0.u[0]);} }
GAME_REPLACE_EXACT(00111580, cdecl, 2, u32, game_effect_store7478_batch5)
{ g_eax=guest_read32(0x73BD64);if(g_eax){b5_load(&g_xmm0,g_esp+4);guest_write32(g_eax+0x74,g_xmm0.u[0]);b5_load(&g_xmm0,g_esp+8);guest_write32(g_eax+0x78,g_xmm0.u[0]);} }
GAME_REPLACE_EXACT(001115A0, cdecl, 2, u32, game_effect_store7c80_batch5)
{ g_eax=guest_read32(0x73BD64);if(g_eax){b5_load(&g_xmm0,g_esp+4);guest_write32(g_eax+0x7C,g_xmm0.u[0]);b5_load(&g_xmm0,g_esp+8);guest_write32(g_eax+0x80,g_xmm0.u[0]);} }
GAME_REPLACE_EXACT(00124F30, cdecl, 1, u32, game_object_flag_float_predicate_batch5)
{ g_eax=guest_read32(g_esp+4);if(guest_read32(g_eax+4)&0x8000000){b5_load(&g_xmm0,g_eax+0x194);if(!b5_below(g_xmm0.f[0],b5_float(guest_read32(0x475CD4)))){g_eax=1;return;}}g_eax=0; }
GAME_REPLACE_EXACT(00164920, cdecl, 1, u32, game_audio_mode_float_store_batch5)
{ g_eax=guest_read32(0x79094C);g_eax-=4;if(g_eax){g_eax-=2;if(g_eax)return;}b5_load(&g_xmm0,0x74C3C8);if(b5_above(g_xmm0.f[0],b5_float(guest_read32(0x475CAC))))return;g_eax=guest_read32(g_esp+4);g_ecx=guest_read32(g_eax+0x2C);b5_load(&g_xmm0,0x4785E4);guest_write32(0x74C3EC,g_ecx);guest_write32(0x74C3C8,g_xmm0.u[0]); }
