#!/usr/bin/env python3
"""
Trajectory compression script.

Compresses training trajectories to fit a token budget using LLM summarization
via the OpenRouter API.

Usage:
    python training/compress.py <input.jsonl> --target-tokens 15250 --output <output.jsonl>

Environment:
    OPENROUTER_API_KEY: Required for summarization API calls.
"""

import argparse
import asyncio
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

import httpx

OPENROUTER_API_URL = "https://openrouter.ai/api/v1/chat/completions"
SUMMARY_PREFIX = "[CONTEXT SUMMARY]:"
SUMMARY_NOTICE = (
    "\n\n[Note: Earlier conversation context has been summarized to fit the context window.]"
)
TRUNCATION_LIMIT = 3000
REQUEST_TIMEOUT = 300.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compress trajectories to fit a token budget using LLM summarization."
    )
    parser.add_argument("input", help="Input JSONL file")
    parser.add_argument("--output", default=None, help="Output file (default: <input>_compressed.jsonl)")
    parser.add_argument("--target-tokens", type=int, default=15250, help="Max tokens per trajectory (default: 15250)")
    parser.add_argument("--summary-tokens", type=int, default=750, help="Target summary size in tokens (default: 750)")
    parser.add_argument("--protect-last", type=int, default=4, help="Keep last N turns verbatim (default: 4)")
    parser.add_argument("--model", default="google/gemini-3-flash-preview", help="Summarization model (default: google/gemini-3-flash-preview)")
    parser.add_argument("--tokenizer", default="moonshotai/Kimi-K2-Thinking", help="HuggingFace tokenizer (default: moonshotai/Kimi-K2-Thinking)")
    parser.add_argument("--workers", type=int, default=4, help="Parallel workers (default: 4)")
    return parser.parse_args()


def resolve_output_path(input_path: str, output_arg: str | None) -> str:
    if output_arg is not None:
        return output_arg
    p = Path(input_path)
    return str(p.with_name(p.stem + "_compressed" + p.suffix))


def load_tokenizer(tokenizer_name: str):
    try:
        from transformers import AutoTokenizer
        print(f"Loading tokenizer: {tokenizer_name}", flush=True)
        return AutoTokenizer.from_pretrained(tokenizer_name)
    except Exception as exc:
        print(f"Error loading tokenizer '{tokenizer_name}': {exc}", file=sys.stderr)
        sys.exit(1)


def count_tokens(tokenizer, text: str) -> int:
    if not text:
        return 0
    return len(tokenizer.encode(text, add_special_tokens=False))


def turn_text(turn: dict[str, Any]) -> str:
    value = turn.get("value") or turn.get("content") or ""
    if isinstance(value, list):
        parts = []
        for item in value:
            if isinstance(item, dict):
                parts.append(item.get("text") or item.get("content") or "")
            else:
                parts.append(str(item))
        return " ".join(parts)
    return str(value)


def get_turns(entry: dict[str, Any]) -> list[dict[str, Any]]:
    return entry.get("conversations") or entry.get("messages") or []


def turns_key(entry: dict[str, Any]) -> str:
    if "conversations" in entry:
        return "conversations"
    return "messages"


def get_role(turn: dict[str, Any]) -> str:
    return turn.get("from") or turn.get("role") or ""


def make_turn_with_value(turn: dict[str, Any], new_value: str) -> dict[str, Any]:
    result = dict(turn)
    if "value" in turn:
        result["value"] = new_value
    elif "content" in turn:
        result["content"] = new_value
    return result


def identify_protected_head(turns: list[dict[str, Any]]) -> list[int]:
    """Return indices of the protected head turns (first system, first human/user, first gpt/assistant, first tool)."""
    seen_roles: set[str] = set()
    head_roles = {"system", "human", "user", "gpt", "assistant", "tool"}
    head_indices: list[int] = []

    for i, turn in enumerate(turns):
        role = get_role(turn)
        normalized = role
        if role in ("human",):
            normalized = "human"
        elif role in ("gpt",):
            normalized = "gpt"

        if normalized in head_roles and normalized not in seen_roles:
            seen_roles.add(normalized)
            head_indices.append(i)

        if seen_roles >= {"system", "human", "gpt"} or seen_roles >= {"system", "user", "assistant"}:
            break

    return head_indices


def build_prompt_for_summary(turns: list[dict[str, Any]]) -> str:
    lines = ["Summarize the following conversation turns concisely, preserving key facts, decisions, and context needed to understand the continuation:\n"]
    for turn in turns:
        role = get_role(turn)
        text = turn_text(turn)
        if len(text) > TRUNCATION_LIMIT:
            text = text[:TRUNCATION_LIMIT] + "...[truncated]"
        lines.append(f"{role.upper()}: {text}")
    return "\n".join(lines)


async def call_openrouter(
    client: httpx.AsyncClient,
    model: str,
    prompt: str,
    api_key: str,
) -> str:
    payload = {
        "model": model,
        "messages": [
            {"role": "user", "content": prompt},
        ],
        "max_tokens": 1024,
    }
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        "HTTP-Referer": "https://github.com/microclaw/mchact",
        "X-Title": "MicroClaw Trajectory Compression",
    }
    response = await client.post(
        OPENROUTER_API_URL,
        json=payload,
        headers=headers,
        timeout=REQUEST_TIMEOUT,
    )
    response.raise_for_status()
    data = response.json()
    return data["choices"][0]["message"]["content"].strip()


async def compress_entry(
    entry: dict[str, Any],
    tokenizer,
    model: str,
    api_key: str,
    client: httpx.AsyncClient,
    semaphore: asyncio.Semaphore,
    target_tokens: int,
    summary_tokens: int,
    protect_last: int,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """
    Returns (compressed_entry, metrics_dict).
    metrics_dict keys: status (skipped|compressed|failed|over_limit), tokens_before, tokens_after, api_calls, error
    """
    key = turns_key(entry)
    turns = get_turns(entry)

    token_counts = [count_tokens(tokenizer, turn_text(t)) for t in turns]
    tokens_before = sum(token_counts)

    if tokens_before <= target_tokens:
        return entry, {
            "status": "skipped",
            "tokens_before": tokens_before,
            "tokens_after": tokens_before,
            "api_calls": 0,
            "error": None,
        }

    head_indices_set = set(identify_protected_head(turns))
    total = len(turns)
    tail_start = max(0, total - protect_last)

    # Build index sets
    tail_indices_set = set(range(tail_start, total))
    middle_indices = [
        i for i in range(total)
        if i not in head_indices_set and i not in tail_indices_set
    ]

    tokens_to_save = tokens_before - target_tokens
    target_to_compress = tokens_to_save + summary_tokens

    # Accumulate middle turns until we have enough to compress
    accumulated = 0
    compress_indices: list[int] = []
    for i in middle_indices:
        compress_indices.append(i)
        accumulated += token_counts[i]
        if accumulated >= target_to_compress:
            break

    if not compress_indices:
        # Nothing to compress; return as-is but flag as over_limit
        return entry, {
            "status": "over_limit",
            "tokens_before": tokens_before,
            "tokens_after": tokens_before,
            "api_calls": 0,
            "error": "No compressible middle turns found",
        }

    turns_to_compress = [turns[i] for i in compress_indices]
    prompt = build_prompt_for_summary(turns_to_compress)

    try:
        async with semaphore:
            summary_text = await asyncio.wait_for(
                call_openrouter(client, model, prompt, api_key),
                timeout=REQUEST_TIMEOUT,
            )
    except asyncio.TimeoutError:
        return entry, {
            "status": "failed",
            "tokens_before": tokens_before,
            "tokens_after": tokens_before,
            "api_calls": 1,
            "error": "API call timed out",
        }
    except Exception as exc:
        return entry, {
            "status": "failed",
            "tokens_before": tokens_before,
            "tokens_after": tokens_before,
            "api_calls": 1,
            "error": str(exc),
        }

    summary_turn = {
        "from": "system",
        "value": f"{SUMMARY_PREFIX} {summary_text}",
    }
    if "role" in turns[0] and "from" not in turns[0]:
        summary_turn = {
            "role": "system",
            "content": f"{SUMMARY_PREFIX} {summary_text}",
        }

    compress_set = set(compress_indices)

    # Build new turns list: head + summary + non-compressed middle + tail
    new_turns: list[dict[str, Any]] = []
    for i in range(total):
        if i in compress_set:
            continue
        turn = turns[i]
        # Inject notice into first system message in head
        if i in head_indices_set and get_role(turn) in ("system",):
            existing = turn_text(turn)
            new_turns.append(make_turn_with_value(turn, existing + SUMMARY_NOTICE))
        else:
            new_turns.append(turn)

    # Insert summary after head turns (before first non-head middle or tail turn)
    head_indices_list = sorted(head_indices_set)
    last_head_pos = head_indices_list[-1] if head_indices_list else -1

    # Find insert position in new_turns (after last head turn)
    insert_pos = 0
    for pos, t in enumerate(new_turns):
        original_idx = None
        # Recompute original index by matching
        for orig_i, orig_t in enumerate(turns):
            if orig_t is t:
                original_idx = orig_i
                break
        if original_idx is not None and original_idx <= last_head_pos:
            insert_pos = pos + 1

    new_turns.insert(insert_pos, summary_turn)

    new_entry = dict(entry)
    new_entry[key] = new_turns

    new_token_counts = [count_tokens(tokenizer, turn_text(t)) for t in new_turns]
    tokens_after = sum(new_token_counts)

    return new_entry, {
        "status": "compressed",
        "tokens_before": tokens_before,
        "tokens_after": tokens_after,
        "api_calls": 1,
        "error": None,
    }


async def run(args: argparse.Namespace) -> None:
    api_key = os.environ.get("OPENROUTER_API_KEY")
    if not api_key:
        print("Error: OPENROUTER_API_KEY environment variable is required.", file=sys.stderr)
        sys.exit(1)

    input_path = args.input
    output_path = resolve_output_path(input_path, args.output)
    metrics_path = Path(output_path).with_suffix(".metrics.json")

    tokenizer = load_tokenizer(args.tokenizer)

    print(f"Reading: {input_path}", flush=True)
    entries: list[dict[str, Any]] = []
    with open(input_path, "r", encoding="utf-8") as f:
        for lineno, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError as exc:
                print(f"Warning: skipping invalid JSON on line {lineno}: {exc}", file=sys.stderr)

    print(f"Loaded {len(entries)} entries", flush=True)

    semaphore = asyncio.Semaphore(args.workers)
    start_time = time.monotonic()

    async with httpx.AsyncClient() as client:
        tasks = [
            compress_entry(
                entry=entry,
                tokenizer=tokenizer,
                model=args.model,
                api_key=api_key,
                client=client,
                semaphore=semaphore,
                target_tokens=args.target_tokens,
                summary_tokens=args.summary_tokens,
                protect_last=args.protect_last,
            )
            for entry in entries
        ]
        results = await asyncio.gather(*tasks)

    elapsed = time.monotonic() - start_time

    compressed_entries: list[dict[str, Any]] = []
    agg_total = len(entries)
    agg_compressed = 0
    agg_skipped = 0
    agg_over_limit = 0
    agg_failed = 0
    agg_tokens_before = 0
    agg_tokens_after = 0
    agg_api_calls = 0
    agg_errors: list[str] = []

    for new_entry, m in results:
        compressed_entries.append(new_entry)
        agg_tokens_before += m["tokens_before"]
        agg_tokens_after += m["tokens_after"]
        agg_api_calls += m["api_calls"]

        status = m["status"]
        if status == "skipped":
            agg_skipped += 1
        elif status == "compressed":
            agg_compressed += 1
        elif status == "over_limit":
            agg_over_limit += 1
        elif status == "failed":
            agg_failed += 1
            if m["error"]:
                agg_errors.append(m["error"])

    print(f"Writing output: {output_path}", flush=True)
    with open(output_path, "w", encoding="utf-8") as f:
        for entry in compressed_entries:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")

    metrics = {
        "total": agg_total,
        "compressed": agg_compressed,
        "skipped": agg_skipped,
        "over_limit": agg_over_limit,
        "failed": agg_failed,
        "tokens_before": agg_tokens_before,
        "tokens_after": agg_tokens_after,
        "tokens_saved": agg_tokens_before - agg_tokens_after,
        "api_calls": agg_api_calls,
        "errors": agg_errors,
        "elapsed_seconds": round(elapsed, 2),
    }

    print(f"Writing metrics: {metrics_path}", flush=True)
    with open(str(metrics_path), "w", encoding="utf-8") as f:
        json.dump(metrics, f, indent=2)

    print(
        f"\nDone: {agg_total} total | {agg_compressed} compressed | "
        f"{agg_skipped} skipped | {agg_over_limit} over_limit | {agg_failed} failed",
        flush=True,
    )
    print(
        f"Tokens: {agg_tokens_before:,} → {agg_tokens_after:,} "
        f"(saved {agg_tokens_before - agg_tokens_after:,})",
        flush=True,
    )
    print(f"API calls: {agg_api_calls} | Time: {elapsed:.1f}s", flush=True)


def main() -> None:
    args = parse_args()
    asyncio.run(run(args))


if __name__ == "__main__":
    main()
