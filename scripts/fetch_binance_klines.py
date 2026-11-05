"""Download Binance klines without third-party dependencies.

Futures (USDⓈ-M perpetual) is the default because Doc Oc trades BTC/USDT
perpetuals. The CSV keeps Binance's raw columns so the teacher can decide
which fields to use later.
"""

import argparse
import csv
import json
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

FUTURES_URL = "https://fapi.binance.com/fapi/v1/klines"
SPOT_URL = "https://api.binance.com/api/v3/klines"
COLUMNS = [
    "open_time",
    "open",
    "high",
    "low",
    "close",
    "volume",
    "close_time",
    "quote_volume",
    "trades",
    "taker_buy_base",
    "taker_buy_quote",
]


def parse_ms(value: str) -> int:
    parsed = datetime.fromisoformat(value)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return int(parsed.timestamp() * 1000)


def fetch_page(base_url: str, symbol: str, interval: str, start_ms: int, limit: int) -> list:
    query = urllib.parse.urlencode(
        {
            "symbol": symbol,
            "interval": interval,
            "startTime": start_ms,
            "limit": limit,
        }
    )
    request = urllib.request.Request(
        f"{base_url}?{query}",
        headers={"User-Agent": "dococ-phase0/0.1"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--symbol", default="BTCUSDT")
    parser.add_argument("--interval", default="1h")
    parser.add_argument("--start", default="2023-01-01")
    parser.add_argument("--end", default=None, help="ISO date; defaults to now")
    parser.add_argument("--market", choices=["futures", "spot"], default="futures")
    parser.add_argument("--out", default="data/btcusdt_1h.csv")
    parser.add_argument("--limit", type=int, default=1500)
    args = parser.parse_args()

    base_url = FUTURES_URL if args.market == "futures" else SPOT_URL
    start_ms = parse_ms(args.start)
    end_ms = parse_ms(args.end) if args.end else int(datetime.now(timezone.utc).timestamp() * 1000)

    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)

    rows_written = 0
    with output.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(COLUMNS)

        cursor = start_ms
        while cursor < end_ms:
            page = fetch_page(base_url, args.symbol, args.interval, cursor, args.limit)
            if not page:
                break
            for row in page:
                if row[0] >= end_ms:
                    break
                writer.writerow(row[: len(COLUMNS)])
                rows_written += 1
            last_open = page[-1][0]
            next_cursor = last_open + 1
            if next_cursor <= cursor:
                raise RuntimeError(f"pagination did not advance at {cursor}")
            cursor = next_cursor
            time.sleep(0.2)

    print(f"wrote {rows_written} rows to {output}")


if __name__ == "__main__":
    main()
