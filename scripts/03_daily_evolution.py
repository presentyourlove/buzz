#!/usr/bin/env python3
"""
03_daily_evolution.py - 每日進化規劃腳本

功能：
- 讀取 02_consensus_ingestion.py 輸出的 project_spec.json
- 基於任務優先級、工作量、依賴關係生成每日進化執行計畫
- 輸出標準化 evolution_plan.json（每日執行藍圖）

CLI 參數：
  --input-file, --output-file, --dry-run, --max-tasks, --priority-threshold
"""

import json
import sys
import argparse
from pathlib import Path
from dataclasses import dataclass, asdict, field
from typing import Dict, List, Any, Optional
from datetime import datetime, timezone
import hashlib


@dataclass
class EvolutionTask:
    """進化任務項目"""
    task_id: str
    title: str
    task_type: str
    priority: str  # P1, P2, P3, P4
    effort: str    # S, M, L
    acceptance_criteria: List[str]
    labels: List[str]
    source_group_id: str
    estimated_hours: float
    depends_on: List[str] = field(default_factory=list)
    risk_level: str = "low"  # low, medium, high
    assigned_agent: str = "Pollen"


@dataclass
class EvolutionPlan:
    """每日進化執行計畫"""
    plan_id: str
    generated_at: str
    source_spec_id: str
    total_tasks: int
    total_estimated_hours: float
    priority_distribution: Dict[str, int]
    effort_distribution: Dict[str, int]
    tasks: List[EvolutionTask]
    execution_order: List[str]  # task_ids in execution order
    metadata: Dict[str, Any]

def load_project_spec(input_file: str) -> Dict[str, Any]:
    """載入 02 的 project_spec.json 輸出"""
    path = Path(input_file)
    if not path.exists():
        raise FileNotFoundError(f"Input file not found: {input_file}")
    return json.loads(path.read_text(encoding="utf-8"))

def estimate_hours(effort: str) -> float:
    """根據工作量等級估算工時"""
    mapping = {"S": 1.0, "M": 4.0, "L": 16.0}
    return mapping.get(effort.upper(), 4.0)

def assess_risk_level(task: Dict[str, Any]) -> str:
    """評估任務風險等級"""
    task_type = task.get("task_type", "")
    priority = task.get("priority", "P4")
    effort = task.get("effort", "S")
    
    # 高風險條件
    if priority == "P1" and effort == "L":
        return "high"
    if task_type in ["tech_debt", "ci_fix"] and effort == "L":
        return "high"
    
    # 中風險條件
    if priority in ["P1", "P2"] and effort in ["M", "L"]:
        return "medium"
    if task_type in ["feature_request", "perf_optimization"] and effort == "M":
        return "medium"
    
    return "low"

def assign_agent(task_type: str) -> str:
    """根據任務類型分配預設特工"""
    mapping = {
        "ci_fix": "Pollen",
        "bug_fix": "Pollen",
        "tech_debt": "Pollen",
        "perf_optimization": "Pollen",
        "feature_request": "Pollen",
        "doc_update": "Honey",
        "investigation": "Honey2",
    }
    return mapping.get(task_type, "Pollen")

def build_dependency_graph(tasks: List[Dict[str, Any]]) -> Dict[str, List[str]]:
    """構建任務依賴圖（基於類型和優先級的隱式依賴）"""
    deps = {}
    task_map = {t["task_id"]: t for t in tasks}
    
    for task in tasks:
        task_id = task["task_id"]
        task_type = task.get("task_type", "")
        priority = task.get("priority", "P4")
        deps[task_id] = []
        
        # CI/建構修復優先於其他任務
        if task_type == "ci_fix":
            continue  # 無依賴，優先執行
        
        # 技術債務可能阻塞特性開發
        if task_type == "feature_request":
            for other_id, other_task in task_map.items():
                if other_task.get("task_type") == "tech_debt" and other_task.get("priority") in ["P1", "P2"]:
                    deps[task_id].append(other_id)
        
        # 效能優化依賴相關 bug 修復
        if task_type == "perf_optimization":
            for other_id, other_task in task_map.items():
                if other_task.get("task_type") == "bug_fix" and "perf" in other_task.get("title", "").lower():
                    deps[task_id].append(other_id)
    
    return deps

def topological_sort(tasks: List[EvolutionTask], deps: Dict[str, List[str]]) -> List[str]:
    """拓撲排序生成執行順序"""
    task_map = {t.task_id: t for t in tasks}
    in_degree = {t.task_id: 0 for t in tasks}
    
    for task_id, dep_list in deps.items():
        for dep in dep_list:
            if dep in in_degree:
                in_degree[task_id] += 1
    
    # 優先級排序作為 tie-breaker
    priority_order = {"P1": 0, "P2": 1, "P3": 2, "P4": 3}
    
    queue = [t.task_id for t in tasks if in_degree[t.task_id] == 0]
    queue.sort(key=lambda x: priority_order.get(task_map[x].priority, 4))
    
    result = []
    while queue:
        current = queue.pop(0)
        result.append(current)
        
        for task_id, dep_list in deps.items():
            if current in dep_list:
                in_degree[task_id] -= 1
                if in_degree[task_id] == 0:
                    queue.append(task_id)
        
        queue.sort(key=lambda x: priority_order.get(task_map[x].priority, 4))
    
    # 檢查循環依賴
    if len(result) != len(tasks):
        # 有循環，退回優先級排序
        return [t.task_id for t in sorted(tasks, key=lambda x: priority_order.get(x.priority, 4))]
    
    return result

def generate_evolution_tasks(spec: Dict[str, Any], max_tasks: Optional[int], priority_threshold: str) -> List[EvolutionTask]:
    """從 project_spec 生成進化任務"""
    priority_rank = {"P1": 1, "P2": 2, "P3": 3, "P4": 4}
    threshold_rank = priority_rank.get(priority_threshold.upper(), 4)
    
    raw_tasks = spec.get("tasks", [])
    evolution_tasks = []
    
    for i, task in enumerate(raw_tasks):
        raw_priority = task.get("priority", "P4")
        if isinstance(raw_priority, int):
            task_priority = f"P{raw_priority}"
        else:
            task_priority = str(raw_priority)
        if priority_rank.get(task_priority, 4) > threshold_rank:
            continue
        
        task_id = task.get("task_id", f"EV-{hashlib.md5(task.get("title", "").encode()).hexdigest()[:8]}")
        effort = task.get("effort", task.get("estimated_effort", "M"))
        
        ev_task = EvolutionTask(
            task_id=task_id,
            title=task.get("title", "Untitled Task"),
            task_type=task.get("task_type", task.get("type", "investigation")),
            priority=str(task_priority) if isinstance(task_priority, int) else task_priority,
            effort=effort,
            acceptance_criteria=task.get("acceptance_criteria", []),
            labels=task.get("labels", []),
            source_group_id=task.get("source_group_id", f"group_{i}"),
            estimated_hours=estimate_hours(effort),
            depends_on=[],  # 稍後填充
            risk_level=assess_risk_level(task),
            assigned_agent=assign_agent(task.get("task_type", ""))
        )
        evolution_tasks.append(ev_task)
    
    # 限制任務數量
    if max_tasks and len(evolution_tasks) > max_tasks:
        evolution_tasks.sort(key=lambda x: priority_rank.get(x.priority, 4))
        evolution_tasks = evolution_tasks[:max_tasks]
    
    return evolution_tasks

def build_evolution_plan(spec: Dict[str, Any], tasks: List[EvolutionTask], max_tasks: Optional[int], priority_threshold: str) -> EvolutionPlan:
    """構建完整的進化計畫"""
    plan_id = f"EVOL-{datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")}"
    
    # 構建依賴並拓撲排序
    task_dicts = [asdict(t) for t in tasks]
    deps = build_dependency_graph(task_dicts)
    
    # 填充 depends_on
    for t in tasks:
        t.depends_on = deps.get(t.task_id, [])
    
    execution_order = topological_sort(tasks, deps)
    
    # 統計
    priority_dist = {}
    effort_dist = {}
    total_hours = 0.0
    
    for t in tasks:
        priority_dist[t.priority] = priority_dist.get(t.priority, 0) + 1
        effort_dist[t.effort] = effort_dist.get(t.effort, 0) + 1
        total_hours += t.estimated_hours
    
    return EvolutionPlan(
        plan_id=plan_id,
        generated_at=datetime.now(timezone.utc).isoformat(),
        source_spec_id=spec.get("spec_id", "unknown"),
        total_tasks=len(tasks),
        total_estimated_hours=round(total_hours, 1),
        priority_distribution=priority_dist,
        effort_distribution=effort_dist,
        tasks=tasks,
    execution_order=execution_order,
        metadata={
            "max_tasks_limit": max_tasks,
            "priority_threshold": priority_threshold,
            "source_signals_count": spec.get("source_signals_count", 0),
            "merged_groups_count": len(spec.get("merged_groups", [])),
            "pipeline_version": "1.0"
        }
    )

def main():
    parser = argparse.ArgumentParser(description="03_daily_evolution.py - 每日進化規劃生成器")
    parser.add_argument("--input-file", required=True, help="輸入的 project_spec.json 路徑（來自 02 輸出）")
    parser.add_argument("--output-file", help="輸出的 evolution_plan.json 路徑（預設輸出到 stdout）")
    parser.add_argument("--dry-run", action="store_true", help="試運行模式，不寫入檔案")
    parser.add_argument("--max-tasks", type=int, help="最大任務數量限制")
    parser.add_argument("--priority-threshold", default="P4", choices=["P1", "P2", "P3", "P4"], help="最低優先級門檻")
    args = parser.parse_args()
    
    try:
        print(f"[INFO] Loading project spec from {args.input_file}", file=sys.stderr)
        spec = load_project_spec(args.input_file)
        
        print(f"[INFO] Generating evolution tasks (threshold={args.priority_threshold}, max_tasks={args.max_tasks})", file=sys.stderr)
        tasks = generate_evolution_tasks(spec, args.max_tasks, args.priority_threshold)
        print(f"[INFO] Generated {len(tasks)} evolution tasks", file=sys.stderr)
        
        plan = build_evolution_plan(spec, tasks, args.max_tasks, args.priority_threshold)
        
        output_json = json.dumps(asdict(plan), ensure_ascii=False, indent=2)
        
        if args.dry_run:
            print(f"[DRY-RUN] Would write evolution plan with {plan.total_tasks} tasks ({plan.total_estimated_hours}h)", file=sys.stderr)
            print(f"[DRY-RUN] Priority distribution: {plan.priority_distribution}", file=sys.stderr)
            print(f"[DRY-RUN] Effort distribution: {plan.effort_distribution}", file=sys.stderr)
            print(f"[DRY-RUN] Execution order: {plan.execution_order}", file=sys.stderr)
            return
        
        if args.output_file:
            output_path = Path(args.output_file)
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(output_json, encoding="utf-8")
            print(f"[INFO] Written evolution plan to {output_path}", file=sys.stderr)
        else:
            print(output_json)
        
        print(f"[INFO] Evolution planning complete: {plan.total_tasks} tasks, {plan.total_estimated_hours}h estimated", file=sys.stderr)
        
    except Exception as e:
        print(f"[ERROR] {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
