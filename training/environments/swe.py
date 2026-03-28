# ---
# name: swe
# class: SweEnv
# description: Software engineering tasks (bug fixing, feature implementation)
# ---
"""
SweEnv — Bundled starter environment for software engineering RL training.

This environment presents software engineering tasks including bug fixing,
feature implementation, and code refactoring. It is designed as a placeholder
compatible with the mchact environment discovery mechanism.

Usage with RL training:
    python training/environments/swe.py serve

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
class SweTask:
    task_id: str
    category: str  # "bug_fix" | "feature" | "refactor"
    title: str
    context: str
    acceptance_criteria: list[str] = field(default_factory=list)
    difficulty: str = "medium"


TASK_BANK: list[SweTask] = [
    SweTask(
        task_id="swe_001",
        category="bug_fix",
        title="Fix off-by-one error in pagination",
        context=(
            "The paginate(items, page, page_size) function returns an empty list when "
            "page=1 because it computes start = page * page_size instead of "
            "start = (page - 1) * page_size."
        ),
        acceptance_criteria=[
            "paginate returns correct slice for page=1",
            "paginate returns correct slice for last page",
            "existing tests pass",
        ],
        difficulty="easy",
    ),
    SweTask(
        task_id="swe_002",
        category="bug_fix",
        title="Handle None input in string normalizer",
        context=(
            "normalize_text(text) raises AttributeError when text is None. "
            "It should return an empty string instead."
        ),
        acceptance_criteria=[
            "normalize_text(None) returns ''",
            "normalize_text('') returns ''",
            "normalize_text('Hello') still works correctly",
        ],
        difficulty="easy",
    ),
    SweTask(
        task_id="swe_003",
        category="feature",
        title="Add retry logic to HTTP client",
        context=(
            "The fetch(url) function makes a single HTTP GET request with no retry. "
            "Add exponential backoff retry with up to 3 attempts on 5xx errors or "
            "network timeouts."
        ),
        acceptance_criteria=[
            "Retries up to 3 times on 5xx status",
            "Retries on connection timeout",
            "Does not retry on 4xx status",
            "Exponential backoff between retries (1s, 2s, 4s)",
        ],
        difficulty="medium",
    ),
    SweTask(
        task_id="swe_004",
        category="feature",
        title="Implement LRU cache decorator",
        context=(
            "Implement a Python decorator @lru_cache(maxsize) that caches function "
            "results with a least-recently-used eviction policy. Must support "
            "cache_info() and cache_clear() on the decorated function."
        ),
        acceptance_criteria=[
            "Caches results for repeated calls with same args",
            "Evicts least recently used entry when maxsize is exceeded",
            "cache_info() returns hits, misses, maxsize, currsize",
            "cache_clear() empties the cache",
        ],
        difficulty="hard",
    ),
    SweTask(
        task_id="swe_005",
        category="refactor",
        title="Extract database logic from god class",
        context=(
            "UserService has grown to 600 lines and mixes business logic with raw SQL. "
            "Extract all database access into a UserRepository class. "
            "UserService should depend only on the repository interface."
        ),
        acceptance_criteria=[
            "UserRepository encapsulates all SQL queries",
            "UserService constructor accepts a repository instance",
            "All existing unit tests still pass",
            "New unit tests for UserRepository are added",
        ],
        difficulty="hard",
    ),
    SweTask(
        task_id="swe_006",
        category="refactor",
        title="Replace mutable default argument with None sentinel",
        context=(
            "Several functions use mutable default arguments such as "
            "def process(data, results=[]):. Replace them with None sentinels "
            "and initialize inside the function body."
        ),
        acceptance_criteria=[
            "No mutable defaults remain in function signatures",
            "Behavior is identical to before",
            "Regression tests pass",
        ],
        difficulty="easy",
    ),
]


class SweEnv:
    """
    Environment for software engineering tasks.

    Agents receive a task description with context and acceptance criteria,
    then must produce code changes (patches, new functions, refactored classes).
    Evaluation checks that acceptance criteria are addressed and code quality
    heuristics are met.
    """

    def __init__(self, seed: int | None = None) -> None:
        self._rng = random.Random(seed)
        self._task_bank = list(TASK_BANK)

    def get_task(self) -> dict[str, Any]:
        """Return a randomly sampled task as a plain dict."""
        task = self._rng.choice(self._task_bank)
        return {
            "task_id": task.task_id,
            "category": task.category,
            "title": task.title,
            "context": task.context,
            "acceptance_criteria": task.acceptance_criteria,
            "difficulty": task.difficulty,
        }

    def evaluate(self, trajectory: dict[str, Any]) -> dict[str, Any]:
        """
        Evaluate a completed trajectory.

        Args:
            trajectory: dict with keys:
                - task_id (str): the task being evaluated
                - patch (str): unified diff or code snippet produced by the agent
                - explanation (str): agent's reasoning / plan

        Returns:
            dict with keys:
                - reward (float): score in [0.0, 1.0]
                - feedback (str): human-readable explanation
        """
        task_id = trajectory.get("task_id", "")
        patch = trajectory.get("patch", "")
        explanation = trajectory.get("explanation", "")

        task = next((t for t in self._task_bank if t.task_id == task_id), None)
        if task is None:
            return {"reward": 0.0, "feedback": f"Unknown task_id: {task_id!r}"}

        if not patch.strip():
            return {"reward": 0.0, "feedback": "No patch/code provided."}

        combined = (patch + " " + explanation).lower()

        # Acceptance criteria coverage
        matched = sum(
            1
            for criterion in task.acceptance_criteria
            if any(word in combined for word in criterion.lower().split()[:3])
        )
        criteria_score = matched / len(task.acceptance_criteria) if task.acceptance_criteria else 1.0

        # Code quality heuristics (simple proxies)
        has_tests = "test" in combined or "assert" in combined
        has_docstring = '"""' in patch or "'''" in patch
        quality_bonus = (0.1 if has_tests else 0.0) + (0.05 if has_docstring else 0.0)

        reward = min(criteria_score + quality_bonus, 1.0)

        feedback = (
            f"Criteria addressed: {matched}/{len(task.acceptance_criteria)} "
            f"({criteria_score:.0%}). "
            f"Tests present: {has_tests}. "
            f"Docstring present: {has_docstring}."
        )

        return {"reward": round(reward, 4), "feedback": feedback}


def _serve() -> None:
    """Minimal stdio server: reads JSON lines, returns JSON lines."""
    env = SweEnv()
    print(json.dumps({"status": "ready", "env": "swe"}), flush=True)
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
    parser = argparse.ArgumentParser(description="SweEnv")
    sub = parser.add_subparsers(dest="cmd")
    sub.add_parser("serve", help="Run stdio JSON-line server")
    args = parser.parse_args()

    if args.cmd == "serve":
        _serve()
    else:
        parser.print_help()
        sys.exit(1)
