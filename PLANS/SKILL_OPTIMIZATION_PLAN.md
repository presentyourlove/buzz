---
title: "Skill Workflow Optimization Plan"
tags: [skill, workflow, optimization]
status: active
created: 2026-10-28
---

# Skill Workflow Optimization Plan

## 1. Executive Summary
This plan details the implementation of workflow optimization for skill management in the codebase located at `C:\私人\ai agent\buzz`.

## 2. Real Architecture & Paths
- Root directory: `C:\私人\ai agent\buzz`
- Skills directory: `C:\私人\ai agent\buzz\.agents\skills`
- CLI commands directory: `C:\私人\ai agent\buzz\crates\buzz-cli\src\commands`
- Existing skills: `desktop-screenshot`, `sprout-cli`
- Key CLI subcommands: `agents.rs`, `channels.rs`, `dms.rs`, `issues.rs`, `messages.rs`, `projects.rs`, `repos.rs`, `workflows.rs`

## 3. Implementation and Testing
- Run test command: `cargo test --package buzz-cli`
- Set Windows encoding: `chcp 65001`

## 4. Rollback and Cache
- Git cleanup: `git checkout . && git clean -fd`
- Cache path: `CLI 無本地快取機制，無需清理`
- Load time threshold: `500ms`
- Onboarding time: `10 minutes`

## 5. Skill Loader & Compatibility Matrix
- **Skill 載入器**：ACP Harness / Desktop App 負責載入 `desktop/src-tauri/src/managed_agents/*.md`，CLI (`buzz-cli`) **無載入器職責**。
- **現有 Skill 定義**：
  - `desktop-screenshot` → `desktop/src-tauri/src/managed_agents/screenshot_skill.md`（Playwright 截圖 + `scripts/post-screenshots.sh` PR 發佈）
  - `sprout-cli` (`buzz-cli`) → `desktop/src-tauri/src/managed_agents/nest_skill.md`（完整 CLI 操作手冊、環境變數、輸出契約、溝通模式）
- **相容性矩陣**（Skill 能力 ↔ CLI 子命令群）：
  | Skill | 核心能力 | 對應 CLI 子命令群 (`crates/buzz-cli/src/commands/`) |
  |-------|----------|-----------------------------------------------------|
  | `desktop-screenshot` | 截圖捕獲、PR 圖床發佈 | 無直接對應（由 `just desktop-screenshot`、`scripts/post-screenshots.sh` 執行） |
  | `buzz-cli` (`sprout-cli`) | 完整 Relay 操作 | `agents.rs`、`channels.rs`、`messages.rs`、`dms.rs`、`workflows.rs`、`feed.rs`、`reactions.rs`、`canvas.rs`、`social.rs`、`repos.rs`、`projects.rs`、`patches.rs`、`issues.rs`、`pr.rs`、`media.rs`、`upload.rs`、`mem.rs`、`pack.rs`、`moderation.rs`、`emoji.rs`、`gifs.rs`、`notes.rs`、`repo_default_branch.rs`、`project_channel.rs`、`event_output.rs` 共 24 命令檔 |
