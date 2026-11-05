"""Build the Laya fine-tune dataset from BTC 1h bars + teacher v1 labels.

Reads data/btcusdt_1h.csv and data/teacher_labels_6k.csv (v1: GARCH(1,1)
Student-t + 3-state HMM + 24h momentum + deterministic rules; beats costs).
Writes data/dataset_btc/{train,test,val}.jsonl in laya-train format:
  state (compact, parity-consistent), questions (action/edge/conviction),
  gold (soft probabilities from teacher p_long/p_short/p_no_action).

Purged splits: chronological 80/10/10 with a 24-bar embargo between
train/test/val boundaries (labels look forward 24 bars).

Usage:
    .venv-quant\Scripts\python.exe scripts\build_dataset.py
"""

import json
import math
import os
import statistics

EMBARGO_BARS = 24
MIN_BARS = 100  # warm-up for volatility/momentum features

STATE_TEMPLATE = (
    "Account: paper. Equity 100000 USD. Position: flat. "
    "Market: BTC/USDT perpetual. Last price {price:.0f}. "
    "24h momentum: {mom24:+.2f}%. "
    "GARCH vol (24h): {vol:.2f}%. "
    "Recent bars: {bars}. "
    "Regime: {regime}. News: {news}."
)

QUESTIONS = {
    "action": {
        "type": "choice",
        "instructions": "What should the desk do?",
        "criteria": ["LONG", "SHORT", "HOLD", "NO_ACTION", "CLOSE"],
    },
    "edge": {
        "type": "noul",
        "instructions": "Does this setup have positive expected value after costs?",
    },
    "conviction": {
        "type": "score",
        "instructions": "How strong is the conviction?",
        "criteria": ["low", "medium", "high"],
    },
}

NEWS_VARIANTS = [
    "spot ETF inflows positive; funding neutral",
    "funding elevated; open interest rising",
    "macro risk-off; equity futures down",
    "regulatory headlines mixed",
    "exchange whale inflows falling",
]


def regime_name(p_down, p_range, p_up):
    if p_up > 0.6:
        return "trending up"
    if p_down > 0.6:
        return "trending down"
    if p_range > 0.6:
        return "ranging"
    return "mixed"


def build_row(bar_idx, closes, bar, label):
    price = float(bar["close"])
    mom24 = (price / closes[bar_idx - 24] - 1.0) * 100.0
    window = closes[bar_idx - 24 : bar_idx + 1]
    rets = [math.log(window[i] / window[i - 1]) for i in range(1, len(window))]
    vol = statistics.stdev(rets) * math.sqrt(24) * 100.0 if len(rets) >= 2 else 0.0
    recent = closes[max(0, bar_idx - 5) : bar_idx + 1]
    bars_text = " | ".join(f"C:{p:.0f}" for p in recent)
    regime = regime_name(float(label["p_down"]), float(label["p_range"]), float(label["p_up"]))
    news = NEWS_VARIANTS[bar_idx % len(NEWS_VARIANTS)]
    state = STATE_TEMPLATE.format(price=price, mom24=mom24, vol=vol, bars=bars_text,
                                  regime=regime, news=news)

    p_long = float(label["p_long"])
    p_short = float(label["p_short"])
    p_no = float(label["p_no_action"])

    # Edge probability from forward 24-bar return (causal label horizon).
    fwd = float(label["fwd_ret_24"])
    edge_prob = min(0.95, max(0.05, 1.0 / (1.0 + math.exp(-fwd * 200.0))))
    conviction = min(2.0, max(0.0, abs(fwd) * 60.0 + max(p_long, p_short) - 0.4))

    action_probs = {"LONG": p_long, "SHORT": p_short, "HOLD": 0.05,
                    "NO_ACTION": max(0.0, p_no - 0.05), "CLOSE": 0.05}
    s = sum(action_probs.values())
    action_probs = {k: round(v / s, 4) for k, v in action_probs.items()}

    return {
        "state": state,
        "questions": QUESTIONS,
        "gold": {
            "action": {"probabilities": action_probs},
            "edge": {"probabilities": {"false": round(1.0 - edge_prob, 4),
                                        "true": round(edge_prob, 4)}},
            "conviction": {"probabilities": {"0": 0.2, "1": 0.3, "2": 0.5},
                           "score": round(conviction, 4)},
        },
    }


def main():
    import pandas as pd

    bars = pd.read_csv("data/btcusdt_1h.csv")
    labels = pd.read_csv("data/teacher_labels_6k.csv")
    bars["ts"] = pd.to_datetime(bars["open_time"], unit="ms", utc=True)
    labels["ts"] = pd.to_datetime(labels["open_time"], utc=True)
    df = labels.merge(
        bars.rename(columns={"close": "close_b"})[["ts", "open", "high", "low", "close_b"]][
            (bars["ts"] >= labels["ts"].iloc[0])
            & (bars["ts"] <= labels["ts"].iloc[-1])],
        on="ts", how="inner")
    assert len(df) == len(labels), f"alignment failed: {len(df)} vs {len(labels)}"
    closes = df["close"].to_list()

    rows = []
    for i in range(MIN_BARS, len(df) - EMBARGO_BARS):
        rows.append(build_row(i, closes, df.iloc[i], df.iloc[i]))

    n = len(rows)
    n_train = int(n * 0.8)
    n_test = int(n * 0.1)
    # Chronological splits with embargo between boundaries.
    train = rows[:n_train]
    test = rows[n_train + EMBARGO_BARS : n_train + EMBARGO_BARS + n_test]
    val = rows[n_train + EMBARGO_BARS + n_test + EMBARGO_BARS :]

    out = "data/dataset_btc"
    os.makedirs(out, exist_ok=True)
    for name, split in [("train", train), ("test", test), ("val", val)]:
        with open(f"{out}/{name}.jsonl", "w") as f:
            for row in split:
                f.write(json.dumps(row) + "\n")
    print(f"train={len(train)} test={len(test)} val={len(val)} total={n}")
    print(f"wrote {out}/{{train,test,val}}.jsonl")


if __name__ == "__main__":
    main()
