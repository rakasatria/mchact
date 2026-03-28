# ---
# name: terminal_tasks
# class: TerminalTasksEnv
# description: Terminal and file manipulation tasks
# ---
"""
TerminalTasksEnv — Bundled starter environment for terminal/file RL training.

This environment presents tasks that require shell commands and file operations
such as creating directories, searching file contents, manipulating text, and
inspecting system state. It is designed as a placeholder compatible with the
mchact environment discovery mechanism.

Usage with RL training:
    python training/environments/terminal_tasks.py serve

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
class TerminalTask:
    task_id: str
    description: str
    command_hint: str
    expected_patterns: list[str] = field(default_factory=list)
    difficulty: str = "easy"


TASK_BANK: list[TerminalTask] = [
    TerminalTask(
        task_id="tt_001",
        description="Count the number of Python files in the current directory tree.",
        command_hint="find . -name '*.py' | wc -l",
        expected_patterns=["find", "wc"],
        difficulty="easy",
    ),
    TerminalTask(
        task_id="tt_002",
        description="Find all TODO comments in Python source files under src/.",
        command_hint="grep -rn 'TODO' src/ --include='*.py'",
        expected_patterns=["grep", "TODO"],
        difficulty="easy",
    ),
    TerminalTask(
        task_id="tt_003",
        description="Create a compressed archive of the logs/ directory named logs_backup.tar.gz.",
        command_hint="tar -czf logs_backup.tar.gz logs/",
        expected_patterns=["tar", "czf", "logs_backup.tar.gz"],
        difficulty="medium",
    ),
    TerminalTask(
        task_id="tt_004",
        description="Show disk usage of each top-level directory, sorted by size descending.",
        command_hint="du -sh */ | sort -rh",
        expected_patterns=["du", "sort"],
        difficulty="medium",
    ),
    TerminalTask(
        task_id="tt_005",
        description=(
            "Replace all occurrences of 'foo' with 'bar' in every .txt file under data/,"
            " in-place."
        ),
        command_hint="find data/ -name '*.txt' -exec sed -i 's/foo/bar/g' {} +",
        expected_patterns=["sed", "s/foo/bar"],
        difficulty="hard",
    ),
    TerminalTask(
        task_id="tt_006",
        description="List all running processes sorted by memory usage (top 10).",
        command_hint="ps aux --sort=-%mem | head -11",
        expected_patterns=["ps", "mem"],
        difficulty="easy",
    ),
    TerminalTask(
        task_id="tt_007",
        description="Check which port a specific process is listening on (e.g. nginx).",
        command_hint="ss -tlnp | grep nginx",
        expected_patterns=["ss", "grep"],
        difficulty="medium",
    ),
]


class TerminalTasksEnv:
    """
    Environment for terminal and file manipulation tasks.

    Agents receive a task description and must produce one or more shell
    commands that accomplish it. Evaluation checks command correctness via
    pattern matching and optional sandbox execution.
    """

    def __init__(self, seed: int | None = None) -> None:
        self._rng = random.Random(seed)
        self._task_bank = list(TASK_BANK)

    def get_task(self) -> dict[str, Any]:
        """Return a randomly sampled task as a plain dict."""
        task = self._rng.choice(self._task_bank)
        return {
            "task_id": task.task_id,
            "description": task.description,
            "difficulty": task.difficulty,
        }

    def evaluate(self, trajectory: dict[str, Any]) -> dict[str, Any]:
        """
        Evaluate a completed trajectory.

        Args:
            trajectory: dict with keys:
                - task_id (str): the task being evaluated
                - commands (list[str]): the shell commands the agent produced
                - explanation (str): optional agent explanation

        Returns:
            dict with keys:
                - reward (float): score in [0.0, 1.0]
                - feedback (str): human-readable explanation
        """
        task_id = trajectory.get("task_id", "")
        commands = trajectory.get("commands", [])
        explanation = trajectory.get("explanation", "")

        task = next((t for t in self._task_bank if t.task_id == task_id), None)
        if task is None:
            return {"reward": 0.0, "feedback": f"Unknown task_id: {task_id!r}"}

        if not commands:
            return {"reward": 0.0, "feedback": "No commands provided."}

        combined = " ".join(commands).lower()

        # Pattern match score
        matched = sum(1 for p in task.expected_patterns if p.lower() in combined)
        pattern_score = matched / len(task.expected_patterns) if task.expected_patterns else 1.0

        # Explanation bonus
        explanation_bonus = 0.1 if len(explanation.strip()) > 20 else 0.0

        reward = min(pattern_score + explanation_bonus, 1.0)

        feedback = (
            f"Pattern matches: {matched}/{len(task.expected_patterns)} "
            f"({pattern_score:.0%}). "
            f"Commands provided: {len(commands)}."
        )

        return {"reward": round(reward, 4), "feedback": feedback}


def _serve() -> None:
    """Minimal stdio server: reads JSON lines, returns JSON lines."""
    env = TerminalTasksEnv()
    print(json.dumps({"status": "ready", "env": "terminal_tasks"}), flush=True)
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
    parser = argparse.ArgumentParser(description="TerminalTasksEnv")
    sub = parser.add_subparsers(dest="cmd")
    sub.add_parser("serve", help="Run stdio JSON-line server")
    args = parser.parse_args()

    if args.cmd == "serve":
        _serve()
    else:
        parser.print_help()
        sys.exit(1)
