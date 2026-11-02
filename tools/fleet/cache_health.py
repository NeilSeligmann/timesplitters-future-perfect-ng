"""Cache hit-rate health checks fed by bounded per-host lookup events."""

from __future__ import annotations

from pathlib import Path

from tools.cache_telemetry import rolling_counts

MIN_SAMPLES = 10


def degraded_caches(
    root: Path | None = None, *, now: float | None = None
) -> list[tuple[str, str, float, float]]:
    """Return (host, cache, baseline rate, observed rate) when observed is under half.

    Compare the previous 24 hours with the current 24 hours. Both sides need at least ten
    lookups to keep sparse samples from generating noisy health alerts.
    """
    findings: list[tuple[str, str, float, float]] = []
    for (host, cache), windows in rolling_counts(root, now=now).items():
        baseline_rate = _rate(windows["baseline"])
        observed_rate = _rate(windows["observed"])
        if baseline_rate is not None and observed_rate is not None:
            if baseline_rate > 0 and observed_rate < baseline_rate / 2:
                findings.append((host, cache, baseline_rate, observed_rate))
    return sorted(findings)


def _rate(value: dict[str, int]) -> float | None:
    hits, misses = value.get("hits"), value.get("misses")
    if (
        isinstance(hits, bool)
        or isinstance(misses, bool)
        or not isinstance(hits, int)
        or not isinstance(misses, int)
        or hits < 0
        or misses < 0
        or hits + misses < MIN_SAMPLES
    ):
        return None
    return hits / (hits + misses)
