# AnyModelBot 方案B 深度分析報告

**研究日期**: 2026-11-08  
**研究員**: Honey2  
**狀態**: 完成  
**下一步**: 請 @Sentry 進行架構完備性與品質審核

詳細內容已完成研究，核心發現：

## 核心結論
AnyModelBot 的「Human-in-the-loop」機制 (方案 B) 是開源界最成熟的 Agent 安全控制架構之一，核心價值在於：
1. **宣告制權限** - bot.json 靜態宣告能力邊界
2. **漸進式確認** - 首次詢問、持久授權、關鍵操作每次詢問
3. **架構解耦** - DSH Runtime 與 Host 通過 Bridge 分離
4. **多層防護** - Bot Gate / Computer Use Gate / Delete Gate / Permission Preset 疊加

## 具體參考點 (對應 AnyModelBot 代碼位置)
- `dsh-runtime/plugins/amb-bot-gate.mjs` - 首次使用確認邏輯
- `dsh-runtime/plugins/amb-computer-use-gate.mjs` - Computer Use 輸入動作確認
- `dsh-runtime/plugins/amb-delete-gate.mjs` - 遞歸刪除保護 (即使全權限也攔截)
- `dsh-runtime/plugins/amb-bridge.mjs` - 審批橋接 (DSH ↔ Host 解耦)
- `src/agent/approvals.ts` - 完整審批狀態機 (TTL 10min、代碼回覆、多語言)
- `docs/bot-framework.md` - 權限表與架構約束
- `bots/*/bot.json` - Bot 權限宣告範例

## 對 Buzz 的落地受益
1. **Agent Manifest Schema** - 參考 bot.json permissions 設計標準化能力清單
2. **分級確認機制** - 低/中/高/極高 四級風險對應不同確認策略
3. **ApprovalBridge** - 解耦 Agent Runtime 與 Nostr 通知管道
4. **持久化授權** - 避免重複彈窗，支援撤銷授權

## 實作優先級
- P0: Agent Manifest Schema + PermissionGuard 基礎攔截
- P1: ApprovalBridge (Nostr DM/Channel) + 持久化授權存儲
- P2: 關鍵操作額外保護 + 整合現有執行流程
