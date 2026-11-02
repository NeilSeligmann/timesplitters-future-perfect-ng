// SPDX-License-Identifier: GPL-3.0-or-later
//! Record identity: the canonical flag line a record is compared against (T1126, T1618).

/// Flags that do not change what the title sees, with how many values follow each.
/// `--unlock-all` is deliberately absent: it changes title-visible progression.
const IGNORED: &[(&str, usize)] = &[
    ("--rust-ports", 1),
    ("--rust-weapon-hands", 0),
    ("--rust-rea-ai", 0),
    ("--perf-log", 0),
    ("--rust-fileio", 0),
    ("--fileio-log", 1),
    ("--record-input", 1),
    ("--replay-input", 1),
    ("--pad-source", 1),
    ("--pad-feed", 1),
    ("--pad-script", 1),
    ("--pad-script-live", 1),
    ("--disc", 1),
    ("--hdd", 1),
    ("--present", 1),
    ("--present-capture", 1),
    ("--present-timeline", 1),
    ("--frame-phase-timer", 1),
    ("--present-no-pace", 0),
    ("--present-hold-ms", 1),
    ("--audio-sink", 1),
    ("--audio-output", 1),
    ("--audio-mute", 0),
    ("--audio-latency-ms", 1),
    ("--audio-min-rate", 1),
    ("--audio-stretch", 1),
    ("--audio-stretch-legacy", 0),
    ("--av-sync-offset-ms", 1),
    ("--audio-hold-ms", 1),
    ("--no-audio-pump", 0),
    ("--live-pipeline", 1),
    ("--live-present-sync", 0),
    ("--live-readback", 0),
    ("--live-blit-verify", 1),
    ("--gpu-live-blit", 0),
    ("--live-frame-hash", 1),
    ("--live-pipeline-cache", 1),
    ("--cpu-profile", 1),
    ("--cpu-profile-wall", 1),
    ("--interactive", 0),
    ("--thread-timeout", 1),
    ("--vblank-owner-waits", 1),
    ("--no-vblank-owner-waits", 0),
    ("--vblank-worker-blanks", 1),
    ("--no-vblank-worker-blanks", 0),
    ("--stop-after-calls", 1),
    ("--trace", 1),
    ("--profile-calls", 0),
    ("--profile-calls-top", 1),
    ("--profile-watch", 1),
    ("--trace-xmv", 0),
    ("--synthetic-pad", 0),
    ("--controllers", 0),
    ("--controller-config", 1),
    ("--controller-mappings", 1),
    ("--controller-save", 1),
    ("--dump-overlay", 1),
    ("--dump-overlay-max", 1),
    ("--dump-xmv-frames", 1),
    ("--capture-xmv-entries", 1),
    ("--gpu-replay-dump", 1),
    ("--gpu-replay-draw-dump", 1),
    ("--snapshot-at-poll", 1),
    ("--stop-at-poll", 1),
    ("--forced-state", 0),
    ("--dump-guest-range", 1),
    ("--dump-guest-dir", 1),
    ("--dump-guest-max-bytes", 1),
    ("--eeprom-widescreen", 0),
    ("--aspect-override", 1),
    ("--poke-at-poll", 1),
    ("--route-wait", 1),
    ("--replay-handover", 0),
    ("--route-wait-event", 1),
    ("--route-event-log", 1),
    ("--route-log-mem", 1),
    ("--voice-log", 1),
    ("--census-icalls", 0),
    ("--census-phases", 1),
    ("--census-window", 1),
    ("--hotkey", 1),
    ("--hotkey-dir", 1),
    ("--shot-dir", 1),
    ("--shot-max", 1),
    ("--shot-max-bytes", 1),
    ("--dump-on-button", 0),
    ("--dump-after-frames", 1),
    ("--dump-button-threshold", 1),
    ("--dump-button-coalesce", 1),
    ("--dump-button-max-pending", 1),
    ("--dump-button-max-dumps", 1),
    ("--dump-button-max-bytes", 1),
    ("--dump-button-start-poll", 1),
    ("--dump-button-after-replay", 0),
    ("--dump-button-idle-every", 1),
    ("--dump-button-max-idle", 1),
    ("--route-nav", 1),
    ("--route-nav-menus", 1),
    ("--route-nav-timing", 1),
    ("--route-nav-record", 1),
    ("--watch-write", 1),
    ("--watch-write-log", 1),
    ("--watch-write-max", 1),
];
/// Flags whose value is an input path: the flag stays, the value becomes `<path>`.
const PATH_VALUED: [&str; 5] = [
    "--gpu-replay",
    "--gpu-replay-undo-viewport",
    "--gpu-replay-window-to-clip",
    "--mount",
    "--seed-xmv-result",
];
const PATH_TOKEN: &str = "<path>";
/// Most tokens either flag line may hold before the C reader gives up.
const MAX_TOKENS: usize = 512;
/// Longest flag line either function accepts (its copy buffer is 2048 bytes with the NUL).
const MAX_LINE: usize = 2047;

/// Appends one token with a separating space when `out` has room under `capacity` (the C buffer
/// size, NUL included), as `put_token` does.
fn put_token(out: &mut String, capacity: usize, token: &str) -> bool {
    let used = out.len();
    let needed = if used > 0 { 2 } else { 1 };
    if capacity.saturating_sub(used) <= token.len() || capacity - used - token.len() < needed {
        return false;
    }
    if used > 0 {
        out.push(' ');
    }
    out.push_str(token);
    true
}

fn canonicalise(args: &[&str], capacity: usize) -> Option<String> {
    if capacity == 0 {
        return None;
    }
    let mut out = String::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        i += 1;
        if arg.starts_with("--rust-ports=") {
            continue;
        }
        if let Some((_, values)) = IGNORED.iter().find(|(name, _)| *name == arg) {
            i += values;
            continue;
        }
        if !put_token(&mut out, capacity, arg) {
            return None;
        }
        if PATH_VALUED.contains(&arg) && i < args.len() {
            i += 1;
            if !put_token(&mut out, capacity, PATH_TOKEN) {
                return None;
            }
        }
    }
    Some(out)
}

/// The canonical identity of a command line (argv[0] and the XBE path skipped).
pub fn identity_flags(argv: &[&str], capacity: usize) -> Option<String> {
    canonicalise(argv.get(2..).unwrap_or(&[]), capacity)
}

fn split_tokens(line: &str) -> Vec<&str> {
    line.split(' ').filter(|t| !t.is_empty()).collect()
}

/// Canonical form of an already recorded `# flags:` line, so per-run output paths compare equal.
pub fn canonical_line(line: &str, capacity: usize) -> Option<String> {
    if line.len() > MAX_LINE {
        return None;
    }
    let tokens = split_tokens(line);
    if tokens.len() > MAX_TOKENS {
        return None;
    }
    canonicalise(&tokens, capacity)
}

/// One flag with its values (a `--` token opens a unit, the tokens after it that do not are values).
fn group_units<'a, 'b>(tokens: &'b [&'a str]) -> Vec<&'b [&'a str]> {
    let mut starts: Vec<usize> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if starts.is_empty() || token.starts_with("--") {
            starts.push(i);
        }
    }
    let mut ends = starts.iter().skip(1).copied().chain([tokens.len()]);
    starts
        .iter()
        .map(|&first| &tokens[first..ends.next().unwrap_or(tokens.len())])
        .collect()
}

/// Appends a unit to a `a b, c` list; a unit that does not fit ends the list with `...`.
fn append_unit(out: &mut String, capacity: usize, unit: &[&str]) {
    if out.ends_with("...") {
        return;
    }
    let used = out.len();
    let need =
        if used > 0 { 2 } else { 0 } + unit.iter().map(|t| t.len()).sum::<usize>() + unit.len() - 1;
    if used + need + 4 > capacity {
        if used + 4 <= capacity {
            out.push_str("...");
        }
        return;
    }
    if used > 0 {
        out.push_str(", ");
    }
    out.push_str(&unit.join(" "));
}

/// The units only in `recorded` and only in `current` (each capped at its capacity like the C
/// buffers) when the two lines differ. A line too long for the C reader, or a capacity under 8,
/// reports no difference (the C function returns false with both lists empty).
pub fn flags_diff(
    recorded: &str,
    current: &str,
    recorded_cap: usize,
    current_cap: usize,
) -> (bool, String, String) {
    let mut only_recorded = String::new();
    let mut only_current = String::new();
    let ta = split_tokens(recorded);
    let tb = split_tokens(current);
    if recorded.len() > MAX_LINE
        || current.len() > MAX_LINE
        || ta.len() > MAX_TOKENS
        || tb.len() > MAX_TOKENS
        || recorded_cap < 8
        || current_cap < 8
    {
        return (false, only_recorded, only_current);
    }
    let ua = group_units(&ta);
    let ub = group_units(&tb);
    let mut matched = vec![false; ub.len()];
    let mut differs = false;
    for unit in &ua {
        match (0..ub.len()).find(|&j| !matched[j] && ub[j] == *unit) {
            Some(j) => matched[j] = true,
            None => {
                append_unit(&mut only_recorded, recorded_cap, unit);
                differs = true;
            }
        }
    }
    for (j, unit) in ub.iter().enumerate() {
        if !matched[j] {
            append_unit(&mut only_current, current_cap, unit);
            differs = true;
        }
    }
    (differs, only_recorded, only_current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_flags_drop_and_paths_blank() {
        let line = "--perf-log --foo --disc /a/b --mount /x --rust-ports=3";
        assert_eq!(canonical_line(line, 2048).unwrap(), "--foo --mount <path>");
    }

    #[test]
    fn diff_lists_changed_units() {
        let (differs, rec, cur) = flags_diff("--a 1 --b", "--a 2 --b", 700, 700);
        assert!(differs);
        assert_eq!((rec.as_str(), cur.as_str()), ("--a 1", "--a 2"));
    }
}
