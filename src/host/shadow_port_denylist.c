/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "shadow_port_denylist.h"
#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <unistd.h>

static int contains_locked(FILE *file, uint32_t va)
{
    char line[128];
    rewind(file);
    while (fgets(line, sizeof line, file) != NULL) {
        size_t len = strlen(line);
        if (len != 0 && line[len - 1] != '\n' && !feof(file)) {
            int c;
            while ((c = fgetc(file)) != '\n' && c != EOF) { }
            continue;
        }
        char *end;
        if (line[0] != '0' || (line[1] != 'x' && line[1] != 'X') ||
            !isxdigit((unsigned char)line[2])) continue;
        errno = 0;
        unsigned long value = strtoul(line, &end, 16);
        if (end == line || errno != 0 || value > UINT32_MAX) continue;
        while (isspace((unsigned char)*end)) ++end;
        if (*end == '\0' && value == va) return 1;
    }
    return ferror(file) ? -1 : 0;
}

int shadow_denylist_contains(const char *path, uint32_t va)
{
    FILE *file = fopen(path, "r");
    if (file == NULL) return errno == ENOENT ? 0 : -1;
    if (flock(fileno(file), LOCK_SH) != 0) { fclose(file); return -1; }
    int result = contains_locked(file, va);
    if (fclose(file) != 0) return -1;
    return result;
}

bool shadow_denylist_add(const char *path, uint32_t va)
{
    int fd = open(path, O_RDWR | O_CREAT | O_APPEND, 0600);
    if (fd < 0) return false;
    FILE *file = fdopen(fd, "a+");
    if (file == NULL) { close(fd); return false; }
    if (flock(fd, LOCK_EX) != 0) { fclose(file); return false; }
    int found = contains_locked(file, va);
    bool ok = found >= 0;
    if (found == 0) {
        if (fseek(file, 0, SEEK_END) != 0) ok = false;
        long length = ok ? ftell(file) : -1;
        if (length < 0) ok = false;
        if (ok && length > 0) {
            if (fseek(file, -1, SEEK_END) != 0) ok = false;
            else if (fgetc(file) != '\n' &&
                     (fseek(file, 0, SEEK_END) != 0 || fputc('\n', file) == EOF)) ok = false;
        }
        if (ok && (fseek(file, 0, SEEK_END) != 0 ||
                   fprintf(file, "0x%08x\n", va) < 0 || fflush(file) != 0)) ok = false;
    }
    if (fclose(file) != 0) ok = false;
    return ok;
}
