#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "host_options.h"
int host_video_configure(options *opts);
int host_video_substitute(const options *opts);
void host_video_note_call(uint32_t callee);
bool host_video_cleanup(void);
