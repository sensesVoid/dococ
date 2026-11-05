"""Print the Python Laya answer for the Rust model-smoke state.

Run with the local multilingual PyTorch checkpoint exported from the same
HuggingFace snapshot used by the ONNX export. The Rust side must ask the same
three questions; compare `action`, `action_confidence`, `edge_probability`,
and `conviction`.
"""

import argparse
import json

import laya

STATE = """\
Account: paper. Equity 100000 USD. Position: flat.
Market: BTC/USDT perpetual. Last price 64000. 4h trend: up.
GARCH conditional volatility: 0.012. Markov regime: trending.
News: spot ETF inflows positive; funding neutral."""

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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("model", help="Local multilingual PyTorch checkpoint directory")
    args = parser.parse_args()

    agent = laya.load(args.model, device="cpu")
    result = agent.predict(STATE, QUESTIONS)
    answers = result["answers"]

    summary = {
        "action": answers["action"]["choice"],
        "action_confidence": answers["action"]["answer_confidence"],
        "action_probabilities": answers["action"]["probabilities"],
        "edge_probability": answers["edge"]["noul"],
        "edge_confidence": answers["edge"]["answer_confidence"],
        "conviction": answers["conviction"]["score"],
        "conviction_confidence": answers["conviction"]["answer_confidence"],
    }
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
