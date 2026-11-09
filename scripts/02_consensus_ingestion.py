#!/usr/bin/env python3
"""
02_consensus_ingestion.py - 共識入庫腳本

功能：
- 讀取 01_collect_signals.py 輸出的 signals JSON
- 訊號去重、同類合併、關聯分析
- 生成標準化 project_spec.json（專案規範/任務藍圖）

CLI 參數：
  --input-file, --output-file, --min-confidence
"""

import json
import sys
import argparse
from pathlib import Path
from datetime import datetime, timezone
from dataclasses import dataclass, asdict
from typing import List, Dict, Any
from collections import defaultdict
import hashlib
import re


@dataclass
class Signal:
    """統一訊號資料結構（對應 01 輸出）"""
    source: str
    type: str
    title: str
    content: str
    metadata: Dict[str, Any]
    timestamp: str
    signal_id: str
    confidence: float = 1.0


@dataclass
class ProjectSpec:
    """專案規範/任務藍圖"""
    spec_id: str
    generated_at: str
    source_signals_count: int
    unique_signals_count: int
    merged_groups: List[Dict[str, Any]]
    tasks: List[Dict[str, Any]]
    priorities: Dict[str, int]
    metadata: Dict[str, Any]


def load_signals(input_file: str) -> Dict[str, Any]:
    """載入 01 的 signals JSON 輸出"""
    path = Path(input_file)
    if not path.exists():
        raise FileNotFoundError(f"Input file not found: {input_file}")
    return json.loads(path.read_text(encoding="utf-8"))


def normalize_signal(sig: Dict[str, Any]) -> Signal:
    """將原始 dict 正規化為 Signal 物件"""
    return Signal(
        source=sig.get("source", "unknown"),
        type=sig.get("type", "unknown"),
        title=sig.get("title", ""),
        content=sig.get("content", ""),
        metadata=sig.get("metadata", {}),
        timestamp=sig.get("timestamp", datetime.now(timezone.utc).isoformat()),
        signal_id=sig.get("signal_id", hashlib.md5(str(sig).encode()).hexdigest()[:12]),
        confidence=sig.get("confidence", 1.0)
    )


def compute_similarity(sig1: Signal, sig2: Signal) -> float:
    """計算兩訊號相似度（簡易版：標題關鍵詞重疊 + 來源一致性）"""
    source_bonus = 0.2 if sig1.source == sig2.source else 0.0
    words1 = set(re.findall(r"\w+", sig1.title.lower()))
    words2 = set(re.findall(r"\w+", sig2.title.lower()))
    if not words1 or not words2:
        return source_bonus
    intersection = len(words1 & words2)
    union = len(words1 | words2)
    title_sim = intersection / union if union > 0 else 0.0
    return min(1.0, title_sim + source_bonus)


def deduplicate_and_merge(signals: List[Signal], min_confidence: float) -> List[Dict[str, Any]]:
    filtered = [s for s in signals if s.confidence >= min_confidence]
    groups = []
    used = set()
    for i, sig in enumerate(filtered):
        if i in used:
            continue
        group = [sig]
        used.add(i)
        for j, other in enumerate(filtered):
            if j in used or i == j:
                continue
            if compute_similarity(sig, other) > 0.6:
                group.append(other)
                used.add(j)
        group_info = {
            "group_id": hashlib.md5(f"{sig.title}{sig.timestamp}".encode()).hexdigest()[:10],
            "representative_title": sig.title,
            "representative_source": sig.source,
            "signal_count": len(group),
            "signal_ids": [s.signal_id for s in group],
            "sources": list(set(s.source for s in group)),
            "types": list(set(s.type for s in group)),
            "merged_content": "\n---\n".join([f"[{s.source}] {s.title}: {s.content[:200]}" for s in group]),
            "avg_confidence": sum(s.confidence for s in group) / len(group),
            "earliest_timestamp": min(s.timestamp for s in group),
            "latest_timestamp": max(s.timestamp for s in group),
        }
        groups.append(group_info)
    return groups


def generate_tasks_from_groups(groups: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    tasks = []
    for group in groups:
        task_type = infer_task_type(group)
        priority = infer_priority(group)
        task = {
            "task_id": f"TASK-{group['group_id'][:8].upper()}",
            "title": group["representative_title"],
            "description": group["merged_content"][:500],
            "type": task_type,
            "priority": priority,
            "source_signals": group["signal_ids"],
            "source_sources": group["sources"],
            "estimated_effort": estimate_effort(group),
            "acceptance_criteria": generate_acceptance_criteria(group),
            "labels": generate_labels(group),
        }
        tasks.append(task)
    return tasks


def infer_task_type(group: Dict[str, Any]) -> str:
    sources = set(group["sources"])
    types = set(group["types"])
    if "github_actions" in sources and "workflow_failure" in types:
        return "ci_fix"
    if "github_actions" in sources and "workflow_slow" in types:
        return "perf_optimization"
    if "nostr" in sources and "agent_mention" in types:
        return "feature_request"
    if "obsidian" in sources and "vault_change" in types:
        return "doc_update"
    if "logs" in sources and "error" in types:
        return "bug_fix"
    if "logs" in sources and "warning" in types:
        return "tech_debt"
    return "investigation"


def infer_priority(group: Dict[str, Any]) -> int:
    sources = set(group["sources"])
    types = set(group["types"])
    count = group["signal_count"]
    avg_conf = group["avg_confidence"]
    if "github_actions" in sources and "workflow_failure" in types and avg_conf > 0.8:
        return 1
    if "logs" in sources and "error" in types and avg_conf > 0.7:
        return 2
    if "nostr" in sources and count >= 2:
        return 2
    if "obsidian" in sources and count >= 2:
        return 3
    if count >= 3:
        return 3
    if avg_conf > 0.8:
        return 3
    return 4


def estimate_effort(group: Dict[str, Any]) -> str:
    count = group["signal_count"]
    sources = len(group["sources"])
    if count >= 3 or sources >= 3:
        return "L"
    if count >= 2 or sources >= 2:
        return "M"
    return "S"


def generate_acceptance_criteria(group: Dict[str, Any]) -> List[str]:
    criteria = []
    task_type = infer_task_type(group)
    if task_type == "ci_fix":
        criteria.extend(["CI pipeline passes on main branch", "No regression in related workflows"])
    elif task_type == "bug_fix":
        criteria.extend(["Error no longer appears in logs", "Related test case added"])
    elif task_type == "feature_request":
        criteria.extend(["Feature implemented and documented", "Integration test passes"])
    elif task_type == "doc_update":
        criteria.extend(["Documentation updated and reviewed", "Links and references valid"])
    else:
        criteria.append("Investigation completed with findings documented")
    return criteria


def generate_labels(group: Dict[str, Any]) -> List[str]:
    labels = [f"source:{s}" for s in group["sources"]]
    labels.append(f"type:{infer_task_type(group)}")
    labels.append(f"priority:P{infer_priority(group)}")
    labels.append(f"effort:{estimate_effort(group)}")
    return labels


def compute_priorities(tasks: List[Dict[str, Any]]) -> Dict[str, int]:
    priorities = defaultdict(int)
    for task in tasks:
        priorities[f"P{task['priority']}"] += 1
    return dict(priorities)


def build_project_spec(
    signals_data: Dict[str, Any],
    groups: List[Dict[str, Any]],
    tasks: List[Dict[str, Any]]
) -> ProjectSpec:
    signals = signals_data.get("signals", [])
    return ProjectSpec(
        spec_id=f"SPEC-{datetime.now(timezone.utc).strftime('%Y%m%d-%H%M%S')}",
        generated_at=datetime.now(timezone.utc).isoformat(),
        source_signals_count=len(signals),
        unique_signals_count=len(groups),
        merged_groups=groups,
        tasks=tasks,
        priorities=compute_priorities(tasks),
        metadata={
            "input_since": signals_data.get("since"),
            "input_sources": signals_data.get("sources_requested"),
            "collection_timestamp": signals_data.get("collected_at"),
            "processor_version": "02_consensus_ingestion.py v1.0",
        }
    )


def main():
    parser = argparse.ArgumentParser(description="02_consensus_ingestion.py - 共識入庫：訊號去重合併並生成專案規範")
    parser.add_argument("--input-file", required=True, help="01 輸出的 signals JSON 檔案路徑")
    parser.add_argument("--output-file", help="輸出 project_spec JSON 檔案路徑（預設 stdout）")
    parser.add_argument("--min-confidence", type=float, default=0.5, help="最低信心門檻（預設 0.5）")
    parser.add_argument("--dry-run", action="store_true", help="僅列出將處理的訊號數量，不寫入檔案")
    args = parser.parse_args()
    
    try:
        signals_data = load_signals(args.input_file)
        raw_signals = signals_data.get("signals", [])
        signals = [normalize_signal(s) for s in raw_signals]
        print(f"[INFO] Loaded {len(signals)} signals from {args.input_file}", file=sys.stderr)
        if args.dry_run:
            print(f"[DRY-RUN] Would process {len(signals)} signals with min_confidence={args.min_confidence}", file=sys.stderr)
            return
        groups = deduplicate_and_merge(signals, args.min_confidence)
        print(f"[INFO] Merged into {len(groups)} groups", file=sys.stderr)
        tasks = generate_tasks_from_groups(groups)
        print(f"[INFO] Generated {len(tasks)} tasks", file=sys.stderr)
        spec = build_project_spec(signals_data, groups, tasks)
        output_json = json.dumps(asdict(spec), ensure_ascii=False, indent=2)
        if args.output_file:
            output_path = Path(args.output_file)
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(output_json, encoding="utf-8")
            print(f"[INFO] Written project spec to {output_path}", file=sys.stderr)
        else:
            print(output_json)
        print(f"[INFO] Consensus ingestion complete: {len(groups)} groups, {len(tasks)} tasks", file=sys.stderr)
    except Exception as e:
        print(f"[ERROR] {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
