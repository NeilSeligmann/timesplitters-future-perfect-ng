/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "hotkey_actions.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "xinput_record.h"

struct hotkey_actions {
    hotkey_action_hooks hooks;
    xinput_host_device source_device;
    unsigned marks_queued, marks_refused, stops_requested, shots_taken, shots_refused, mu_toggles, mu_refused, leaks_trimmed, leaks_ignored;
};

hotkey_actions *hotkey_actions_create(xinput_host_device source_device, const hotkey_action_hooks *hooks)
{
    hotkey_actions *actions = calloc(1u, sizeof *actions);
    if (actions == NULL) return NULL;
    actions->source_device = source_device;
    if (hooks != NULL) actions->hooks = *hooks;
    return actions;
}

void hotkey_actions_destroy(hotkey_actions *actions)
{
    free(actions);
}

void hotkey_actions_fire(unsigned number, const char *label, uint64_t poll, void *context)
{
    hotkey_actions *actions = context;
    if (actions == NULL) return;
    switch (xinput_hotkey_action_of(label)) {
    case XINPUT_HOTKEY_ACTION_MARK:
        if (actions->hooks.mark != NULL && actions->hooks.mark(actions->hooks.user)) {
            actions->marks_queued++;
        } else {
            actions->marks_refused++;
            fprintf(stderr, "hotkey (T1632): 'mark' #%u at host poll %llu ignored, no --record-input recording is open\n", number,
                    (unsigned long long)poll);
        }
        break;
    case XINPUT_HOTKEY_ACTION_STOP:
        actions->stops_requested++;
        fprintf(stderr, "hotkey (T1632): 'stop' #%u at host poll %llu, ending the run cleanly\n", number, (unsigned long long)poll);
        if (actions->hooks.stop != NULL) actions->hooks.stop(actions->hooks.user, "stop hotkey (T1632)");
        break;
    case XINPUT_HOTKEY_ACTION_SHOT:
        if (actions->hooks.shot != NULL && actions->hooks.shot(actions->hooks.user, number, label, poll)) {
            actions->shots_taken++;
        } else {
            actions->shots_refused++;
        }
        break;
    case XINPUT_HOTKEY_ACTION_MU:
        if (actions->hooks.mu != NULL && actions->hooks.mu(actions->hooks.user)) {
            actions->mu_toggles++;
        } else {
            actions->mu_refused++;
            fprintf(stderr, "hotkey (T2225): 'mu' #%u at host poll %llu refused\n", number, (unsigned long long)poll);
        }
        break;
    default:
        break;
    }
}

void hotkey_actions_leak(xinput_hotkey_device device, const char *name, uint64_t poll, bool begin, void *context)
{
    hotkey_actions *actions = context;
    if (actions == NULL) return;
    const xinput_host_device key_device = device == XINPUT_HOTKEY_DEVICE_PAD ? XINPUT_HOST_GAMEPAD : XINPUT_HOST_KEYBOARD;
    xinput_record_effect effect;
    uint16_t digital = 0u;
    int analog = -1, stick = -1;
    if (key_device != actions->source_device || !xinput_host_key_effect(key_device, name, &digital, &analog, &stick)) {
        if (begin) actions->leaks_ignored++;
        return;
    }
    effect.digital_mask = digital;
    effect.analog = analog;
    effect.stick = stick;
    if (begin) {
        actions->leaks_trimmed++;
        xinput_record_suppress_begin(&effect, poll);
    } else {
        xinput_record_suppress_end(&effect, poll);
    }
}
