"""Basic integrity report for a Binance klines CSV."""

import csv
import sys
from datetime import datetime, timezone
from pathlib import Path

STEP_MS = 3_600_000


def iso(ms: int) -> str:
    return datetime.fromtimestamp(ms / 1000, tz=timezone.utc).isoformat()


def main() -> None:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "data/btcusdt_1h.csv")
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))

    if not rows:
        print("empty")
        return

    times = [int(row["open_time"]) for row in rows]
    gaps = [(left, right) for left, right in zip(times, times[1:]) if right - left != STEP_MS]
    missing = sum((right - left) // STEP_MS - 1 for left, right in gaps)
    duplicates = len(times) - len(set(times))
    bad_ohlc = sum(
        not (float(row["low"]) <= float(row["open"]) <= float(row["high"]))
        or not (float(row["low"]) <= float(row["close"]) <= float(row["high"]))
        for row in rows
    )
    closes = [float(row["close"]) for row in rows]

    print(f"path={path}")
    print(f"rows={len(rows)} first={iso(times[0])} last={iso(times[-1])}")
    print(f"duplicates={duplicates} gaps={len(gaps)} missing_bars={missing}")
    if gaps:
        worst = max(gaps, key=lambda pair: pair[1] - pair[0])
        print(
            f"largest_gap={iso(worst[0])} -> {iso(worst[1])} "
            f"hours={(worst[1] - worst[0]) / STEP_MS:.0f}"
        )
    print(f"bad_ohlc={bad_ohlc} close_min={min(closes):.2f} close_max={max(closes):.2f}")


if __name__ == "__main__":
    main()
