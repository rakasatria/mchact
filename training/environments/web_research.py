# ---
# name: web_research
# class: WebResearchEnv
# description: Web research and information synthesis tasks
# ---
"""
WebResearchEnv — Bundled starter environment for web research RL training.

This environment presents tasks that require searching the web and synthesizing
information from multiple sources into a coherent answer. It is designed as a
placeholder compatible with the mchact environment discovery mechanism.

Usage with RL training:
    python training/environments/web_research.py serve

    Or integrate with atroposlib by subclassing atroposlib.BaseEnv and delegating
    to get_task() / evaluate() defined here.
"""

from __future__ import annotations

import argparse
import json
import random
import sys
from dataclasses import dataclass, field
from typing import Any


@dataclass
class Task:
    task_id: str
    query: str
    expected_topics: list[str] = field(default_factory=list)
    difficulty: str = "medium"


TASK_BANK: list[Task] = [
    Task(
        task_id="wr_001",
        query="What are the key differences between RAG and fine-tuning for LLMs?",
        expected_topics=["retrieval-augmented generation", "fine-tuning", "LLM", "trade-offs"],
        difficulty="medium",
    ),
    Task(
        task_id="wr_002",
        query="Summarize recent advances in protein folding prediction since AlphaFold2.",
        expected_topics=["protein folding", "AlphaFold", "structural biology", "ML"],
        difficulty="hard",
    ),
    Task(
        task_id="wr_003",
        query="What is the current state of fusion energy research?",
        expected_topics=["nuclear fusion", "ITER", "tokamak", "energy"],
        difficulty="medium",
    ),
    Task(
        task_id="wr_004",
        query="How does the transformer architecture differ from recurrent neural networks?",
        expected_topics=["transformer", "RNN", "attention", "sequential modeling"],
        difficulty="easy",
    ),
    Task(
        task_id="wr_005",
        query="What open-source licenses are compatible with commercial use?",
        expected_topics=["MIT", "Apache 2.0", "BSD", "GPL", "open source licensing"],
        difficulty="easy",
    ),
]


class WebResearchEnv:
    """
    Environment for web research and information synthesis tasks.

    Agents receive a research question and must produce a well-sourced,
    synthesized answer. Evaluation rewards coverage of expected topics,
    citation quality, and answer coherence.
    """

    def __init__(self, seed: int | None = None) -> None:
        self._rng = random.Random(seed)
        self._task_bank = list(TASK_BANK)

    def get_task(self) -> dict[str, Any]:
        """Return a randomly sampled task as a plain dict."""
        task = self._rng.choice(self._task_bank)
        return {
            "task_id": task.task_id,
            "query": task.query,
            "difficulty": task.difficulty,
        }

    def evaluate(self, trajectory: dict[str, Any]) -> dict[str, Any]:
        """
        Evaluate a completed trajectory.

        Args:
            trajectory: dict with keys:
                - task_id (str): the task being evaluated
                - answer (str): the agent's synthesized answer
                - sources (list[str]): URLs or references cited

        Returns:
            dict with keys:
                - reward (float): score in [0.0, 1.0]
                - feedback (str): human-readable explanation
        """
        task_id = trajectory.get("task_id", "")
        answer = trajectory.get("answer", "")
        sources = trajectory.get("sources", [])

        task = next((t for t in self._task_bank if t.task_id == task_id), None)
        if task is None:
            return {"reward": 0.0, "feedback": f"Unknown task_id: {task_id!r}"}

        # Topic coverage score
        answer_lower = answer.lower()
        matched = sum(1 for topic in task.expected_topics if topic.lower() in answer_lower)
        coverage = matched / len(task.expected_topics) if task.expected_topics else 1.0

        # Citation bonus (capped at 0.2 extra)
        citation_bonus = min(len(sources) * 0.05, 0.2)

        # Minimum length heuristic
        length_ok = len(answer.split()) >= 50
        length_penalty = 0.0 if length_ok else 0.3

        reward = min(coverage + citation_bonus - length_penalty, 1.0)
        reward = max(reward, 0.0)

        feedback = (
            f"Topic coverage: {matched}/{len(task.expected_topics)} "
            f"({coverage:.0%}). "
            f"Sources provided: {len(sources)}. "
            f"Word count: {len(answer.split())}."
        )

        return {"reward": round(reward, 4), "feedback": feedback}


def _serve() -> None:
    """Minimal stdio server: reads JSON lines, returns JSON lines."""
    env = WebResearchEnv()
    print(json.dumps({"status": "ready", "env": "web_research"}), flush=True)
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            print(json.dumps({"error": f"Invalid JSON: {exc}"}), flush=True)
            continue

        command = request.get("command")
        if command == "get_task":
            print(json.dumps(env.get_task()), flush=True)
        elif command == "evaluate":
            trajectory = request.get("trajectory", {})
            print(json.dumps(env.evaluate(trajectory)), flush=True)
        else:
            print(json.dumps({"error": f"Unknown command: {command!r}"}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="WebResearchEnv")
    sub = parser.add_subparsers(dest="cmd")
    sub.add_parser("serve", help="Run stdio JSON-line server")
    args = parser.parse_args()

    if args.cmd == "serve":
        _serve()
    else:
        parser.print_help()
        sys.exit(1)
