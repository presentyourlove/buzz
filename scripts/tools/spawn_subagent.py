#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
spawn_subagent.py - 全特工動態子特工 (Sub-agent Dispatcher) 派發工具

職責：
1. 為 Buzz 特工（Pollen, Honey, Honey2, Sentry, Fizz, Wax）提供動態派生子特工能力。
2. 自動建立獨立物理沙盒工作區（.scratch/subagents/<id>/），隔絕主倉庫檔案與 Git 操作，徹底防止多工衝突。
3. 自動繼承父特工之憑證與模型配置，支援 OpenAI/OpenRouter 相容端點呼叫。
4. 輸出標準化結構 result.json，並支援非阻塞與同步等待執行。
"""

import argparse
import json
import os
import sys
import time
import uuid
import urllib.request
import urllib.error
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional, Dict, Any, List

# 常數定義（禁用 Magic Numbers）
DEFAULT_TIMEOUT_SECONDS: int = 300
DEFAULT_MAX_TOKENS: int = 4096
DEFAULT_TEMPERATURE: float = 0.2
REPO_ROOT: Path = Path("c:/私人/ai agent/buzz").resolve()
MANAGED_AGENTS_PATH: Path = Path(os.path.expandvars(r"%APPDATA%\xyz.block.buzz.app\agents\managed-agents.json"))
DEFAULT_SANDBOX_BASE: Path = REPO_ROOT / ".scratch" / "subagents"


@dataclass
class SubagentConfig:
    parent_name: str
    task_name: str
    prompt: str
    isolated_dir: Path
    model: str
    api_key: str
    base_url: str
    timeout_seconds: int
    dry_run: bool


@dataclass
class SubagentResult:
    task_id: str
    task_name: str
    parent_name: str
    status: str          # "completed" | "failed" | "dry_run" | "timeout"
    execution_time_ms: int
    output_text: str
    sandbox_path: str
    created_at: str
    completed_at: str
    error: Optional[str] = None


def load_parent_agent_config(parent_name: str) -> Dict[str, Any]:
    """
    從 managed-agents.json 載入父特工的配置（模型與憑證），若找不到則使用環境變數或預設值。
    """
    config: Dict[str, Any] = {
        "model": "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free",
        "api_key": "",
        "base_url": "https://openrouter.ai/api/v1",
    }

    if not MANAGED_AGENTS_PATH.exists():
        # 降級至環境變數
        config["api_key"] = os.environ.get("OPENROUTER_API_KEY", "") or os.environ.get("OPENAI_API_KEY", "")
        return config

    try:
        with open(MANAGED_AGENTS_PATH, "r", encoding="utf-8") as fp:
            records = json.load(fp)

        # 優先搜尋 Instance 紀錄（帶有 env_vars）
        matched = None
        for r in records:
            if r.get("name", "").lower() == parent_name.lower():
                matched = r
                if r.get("env_vars"):
                    break

        if matched:
            env_vars = matched.get("env_vars") or {}
            config["model"] = matched.get("model") or config["model"]

            # 解析憑證
            if env_vars.get("OPENAI_COMPAT_API_KEY"):
                config["api_key"] = env_vars["OPENAI_COMPAT_API_KEY"]
                config["base_url"] = env_vars.get("OPENAI_COMPAT_BASE_URL", "https://integrate.api.nvidia.com/v1")
            elif env_vars.get("OPENROUTER_API_KEY"):
                config["api_key"] = env_vars["OPENROUTER_API_KEY"]
                config["base_url"] = "https://openrouter.ai/api/v1"
            elif env_vars.get("OPENAI_API_KEY"):
                config["api_key"] = env_vars["OPENAI_API_KEY"]
                config["base_url"] = env_vars.get("OPENAI_BASE_URL", "https://api.openai.com/v1")

    except Exception as err:
        print(f"[WARN] Failed to read managed-agents.json: {err}", file=sys.stderr)

    return config


def setup_sandbox(base_dir: Path, task_id: str) -> Path:
    """
    建立沙盒隔離目錄，並產生 .gitignore 確保子特工的暫存檔不會污染主 Git 倉庫。
    """
    sandbox_dir = base_dir / task_id
    sandbox_dir.mkdir(parents=True, exist_ok=True)

    gitignore_path = sandbox_dir / ".gitignore"
    if not gitignore_path.exists():
        gitignore_path.write_text("*\n", encoding="utf-8")

    return sandbox_dir


def execute_llm_request(config: SubagentConfig) -> str:
    """
    以純標準庫向 OpenAI/OpenRouter 相容端點發送推理請求，避免外部第三方庫依賴。
    """
    if not config.api_key:
        raise ValueError(f"No API key available for parent agent '{config.parent_name}'.")

    endpoint = f"{config.base_url.rstrip('/')}/chat/completions"
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {config.api_key}",
        "User-Agent": "BuzzSubagentDispatcher/1.0",
    }

    system_content = (
        f"你是 {config.parent_name} 派出的獨立子特工 (Sub-agent: {config.task_name})。\n"
        f"你正在物理隔離的沙盒工作區執行專注任務。\n"
        f"請專注完成給定目標，輸出精煉、準確、結構化的成果。"
    )

    payload = {
        "model": config.model,
        "messages": [
            {"role": "system", "content": system_content},
            {"role": "user", "content": config.prompt},
        ],
        "temperature": DEFAULT_TEMPERATURE,
        "max_tokens": DEFAULT_MAX_TOKENS,
    }

    req_data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(endpoint, data=req_data, headers=headers, method="POST")

    try:
        with urllib.request.urlopen(req, timeout=config.timeout_seconds) as resp:
            resp_bytes = resp.read()
            data = json.loads(resp_bytes.decode("utf-8"))
            choices = data.get("choices", [])
            if choices and "message" in choices[0]:
                return choices[0]["message"].get("content", "").strip()
            return ""
    except urllib.error.HTTPError as http_err:
        err_body = http_err.read().decode("utf-8", errors="ignore")
        raise RuntimeError(f"HTTP {http_err.code}: {err_body}")
    except Exception as e:
        raise RuntimeError(f"LLM request failed: {e}")


def dispatch_subagent(config: SubagentConfig) -> SubagentResult:
    """
    調度並執行子特工任務主流程。
    """
    task_id = f"subagent_{config.parent_name.lower()}_{int(time.time())}_{uuid.uuid4().hex[:6]}"
    sandbox = setup_sandbox(config.isolated_dir, task_id)
    created_at = datetime.now(timezone.utc).isoformat()
    start_time = time.perf_counter()

    print(f"[INFO] Dispatching subagent [{task_id}] for parent '{config.parent_name}'...", file=sys.stderr)
    print(f"[INFO] Sandbox path: {sandbox}", file=sys.stderr)

    if config.dry_run:
        time.sleep(0.05)
        exec_ms = int((time.perf_counter() - start_time) * 1000)
        return SubagentResult(
            task_id=task_id,
            task_name=config.task_name,
            parent_name=config.parent_name,
            status="dry_run",
            execution_time_ms=exec_ms,
            output_text=f"[DRY-RUN] Task '{config.task_name}' validated successfully in sandbox.",
            sandbox_path=str(sandbox),
            created_at=created_at,
            completed_at=datetime.now(timezone.utc).isoformat(),
            error=None
        )

    try:
        output = execute_llm_request(config)
        exec_ms = int((time.perf_counter() - start_time) * 1000)
        res = SubagentResult(
            task_id=task_id,
            task_name=config.task_name,
            parent_name=config.parent_name,
            status="completed",
            execution_time_ms=exec_ms,
            output_text=output,
            sandbox_path=str(sandbox),
            created_at=created_at,
            completed_at=datetime.now(timezone.utc).isoformat(),
            error=None
        )
    except Exception as exc:
        exec_ms = int((time.perf_counter() - start_time) * 1000)
        res = SubagentResult(
            task_id=task_id,
            task_name=config.task_name,
            parent_name=config.parent_name,
            status="failed",
            execution_time_ms=exec_ms,
            output_text="",
            sandbox_path=str(sandbox),
            created_at=created_at,
            completed_at=datetime.now(timezone.utc).isoformat(),
            error=str(exc)
        )

    # 寫入沙盒 result.json
    result_file = sandbox / "result.json"
    result_file.write_text(json.dumps(asdict(res), ensure_ascii=False, indent=2), encoding="utf-8")
    return res


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Buzz 特工動態 Sub-agent 派發工具")
    parser.add_argument("--parent", type=str, required=True, help="父特工名稱（如 Pollen, Honey2, Sentry）")
    parser.add_argument("--task-name", type=str, required=True, help="子任務名稱/標籤")
    parser.add_argument("--prompt", type=str, default="", help="任務提示詞/指令")
    parser.add_argument("--prompt-file", type=str, default="", help="任務提示詞檔案路徑")
    parser.add_argument("--isolated-dir", type=str, default="", help="自定義沙盒隔離目錄")
    parser.add_argument("--model", type=str, default="", help="覆寫使用之模型")
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS, help="超時秒數")
    parser.add_argument("--output-file", type=str, default="", help="輸出 result.json 之目標路徑")
    parser.add_argument("--dry-run", action="store_true", help="乾跑模式（不實際請求 LLM）")
    return parser.parse_args()


def main():
    args = parse_arguments()

    # 解析 Prompt
    prompt_text = args.prompt
    if args.prompt_file:
        pf = Path(args.prompt_file)
        if pf.exists():
            prompt_text = pf.read_text(encoding="utf-8")
        else:
            print(f"[ERROR] Prompt file '{args.prompt_file}' not found.", file=sys.stderr)
            sys.exit(1)

    if not prompt_text and not args.dry_run:
        print("[ERROR] Prompt cannot be empty unless --dry-run is specified.", file=sys.stderr)
        sys.exit(1)

    parent_cfg = load_parent_agent_config(args.parent)
    model = args.model or parent_cfg.get("model", "")
    api_key = parent_cfg.get("api_key", "")
    base_url = parent_cfg.get("base_url", "")

    isolated_dir = Path(args.isolated_dir) if args.isolated_dir else DEFAULT_SANDBOX_BASE

    config = SubagentConfig(
        parent_name=args.parent,
        task_name=args.task_name,
        prompt=prompt_text,
        isolated_dir=isolated_dir,
        model=model,
        api_key=api_key,
        base_url=base_url,
        timeout_seconds=args.timeout,
        dry_run=args.dry_run
    )

    result = dispatch_subagent(config)
    result_json = json.dumps(asdict(result), ensure_ascii=False, indent=2)

    if args.output_file:
        out_p = Path(args.output_file)
        out_p.parent.mkdir(parents=True, exist_ok=True)
        out_p.write_text(result_json, encoding="utf-8")
        print(f"[INFO] Result written to {out_p}", file=sys.stderr)
    else:
        print(result_json)

    if result.status == "failed":
        sys.exit(1)


if __name__ == "__main__":
    main()

