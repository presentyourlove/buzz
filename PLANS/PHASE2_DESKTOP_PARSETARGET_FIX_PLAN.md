---
title: "Phase 2 Desktop parseTarget 相容性修正計畫"
tags: [phase2, reminders, desktop, cli, compatibility, fix]
status: active
created: 2026-10-31
---

# Phase 2 Desktop parseTarget 相容性修正計畫

## 背景與問題

@Sentry Phase 2 條件式驗收報告指出關鍵阻礙：
- **Desktop `parseTarget`（`reminderService.ts:96-115`）** 原要求 target **同時包含 4 個字串欄位**（`eventId`、`channelId`、`preview`、`authorPubkey`），缺一不可
- **CLI（`reminders.rs:70-88`）** 允許任意組合（只給 channel、只給 event-id、或全給）
- 導致 CLI 建立的大多數提醒在 Desktop **被忽略不顯示**

## 解決方案（採用 @Sentry 建議方案 A）

**放寬 Desktop `parseTarget`**：只要求 `channelId` 或 `eventId` 至少一個存在，其餘缺失視為空字串。這與 NIP-ER 彈性目標結構一致，且一次修正相容所有 CLI 使用模式。

### 已完成的代碼變更（工作樹中，未提交）

**檔案**：`desktop/src/features/reminders/lib/reminderService.ts`（第 96-122 行）

```typescript
function parseTarget(value: unknown): ReminderTarget | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  const t = value as Record<string, unknown>;

  // Accept target if it has at least channelId OR eventId as strings.
  // Missing fields default to empty string to maintain ReminderTarget shape.
  // This matches NIP-ER's flexible target structure where either channelId
  // or eventId (or both) may be provided.
  const eventId = typeof t.eventId === "string" ? t.eventId : "";
  const channelId = typeof t.channelId === "string" ? t.channelId : "";
  const preview = typeof t.preview === "string" ? t.preview : "";
  const authorPubkey = typeof t.authorPubkey === "string" ? t.authorPubkey : "";

  // Require at least one identifier to consider it a valid target
  if (!eventId && !channelId) {
    return null;
  }

  return {
    eventId,
    channelId,
    preview,
    authorPubkey,
  };
}
```

### 相關架構說明

1. **`ReminderTarget` 型別**（`reminderTypes.ts:3-12`）維持 4 欄位皆為 `string`，`parseTarget` 現在保證回傳完整形狀
2. **`hasNavigableTarget`**（`reminderNavigation.ts:23-32`）仍要求 `channelId`、`eventId`、`authorPubkey` 三者非空才判定為可導航 —— **部分 target 仍會顯示在列表，但不可點擊跳轉**，符合設計意圖（與 note-only 提醒同等待遇）
3. **Desktop 原生「稍後提醒」**（`MessageRow.tsx:231-238`）一律建構完整 4 欄位，不受影響
4. **CLI 建立提醒**（`reminders.rs:70-88`）條件性填入欄位，現經 `parseTarget` 鬆綁後可正常解析顯示

## 驗收標準

| 項目 | 驗證方式 | 預期結果 |
|------|----------|----------|
| TypeScript 型別檢查 | `cd desktop && npx tsc --noEmit` | 0 錯誤 |
| CLI 建立部分 target 提醒 | `buzz reminders create "test" --at "+5m" --channel <uuid>` | 成功建立，Desktop 列表可見 |
| CLI 建立完整 target 提醒 | `buzz reminders create "test" --at "+5m" --channel <uuid> --event-id <eid> --preview "..." --author-pubkey <pk>` | 成功建立，Desktop 列表可見且可點擊導航 |
| Desktop 原生「稍後提醒」 | UI 右鍵訊息 → Remind me later | 正常運作，可導航 |
| 現有測試通過 | `cargo test --package buzz-cli` / `pnpm test` | 全綠 |

## 回滾機制

```bash
cd "c:\私人\ai agent\buzz" && git restore desktop/src/features/reminders/lib/reminderService.ts
```

## 後續行動

1. **@Sentry** 複審：確認修正解決相容性缺口，且不破壞現有導航邏輯
2. 老闆/@Fizz 確認合併
3. 納入 Phase 2 完整補丁（`C:\Users\user\.buzz\OUTBOX\reminders_phase2_desktop.patch` 需重新生成含此修正）

---

**狀態**：代碼修正已就位（工作樹），待 @Sentry 複審通過後提交。
