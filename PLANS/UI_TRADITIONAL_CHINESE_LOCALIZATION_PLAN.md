---
title: "Buzz 介面繁體中文化實施計畫書"
tags: [localization, i18n, traditional-chinese, ui]
status: active
created: 2026-10-24
---

# Buzz 介面繁體中文化實施計畫書

## 專案概述

本計畫旨在將 Buzz 桌面應用程式與 Web 前端的側邊欄、按鈕、對話框等 UI 介面文字從英文轉換為繁體中文（台灣慣用語），作為 Buzz 第一次自我進化的里程碑任務。

## 範疇

### 包含範圍
- **Desktop 應用程式** (`C:\私人\ai agent\buzz\desktop\src\`)
  - 側邊欄導航與區段標題
  - 側邊欄操作選單（區段操作、頻道右鍵選單）
  - 建立頻道/論壇對話框
  - 新增社群對話框
  - 區段建立/重命名/刪除對話框
  - 離開頻道/刪除專案確認對話框
  - 搜尋/頂欄搜尋介面
  - 頻道活動彈出視窗
  - 未讀訊息指示器
  - 個人資料卡片
  - 專案側邊欄區段

- **Web 前端** (`C:\私人\ai agent\buzz\web\src\`)
  - 目前 Web 前端較簡略，主要為邀請/專案相關頁面，同步處理

### 不包含範圍
- 後端 API 回應訊息
- 錯誤日誌與開發者工具訊息
- Markdown 文件內容（README、CHANGELOG 等）
- 單元測試斷言字串

## 英文原文字彙對照表

### 1. 側邊欄主導航

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Inbox | 收件箱 | `AppSidebarPinnedHeader.tsx:115,119` | Tooltip 與 Label |
| Pulse | 脈動 | `AppSidebarPinnedHeader.tsx:136,140` | FeatureGate 保護 |
| Projects | 專案 | `AppSidebarPinnedHeader.tsx:150,154` | FeatureGate 保護 |
| Agents | 代理人 | `AppSidebarPinnedHeader.tsx:164,168` | |
| Workflows | 工作流 | `AppSidebarPinnedHeader.tsx:178,182` | FeatureGate 保護 |

### 2. 側邊欄區段標題

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Starred | 已加星標 | `AppSidebar.tsx:610` | ChannelGroupSection title |
| Channels | 頻道 | `AppSidebar.tsx:713` | ChannelGroupSection title |
| Forums | 論壇 | `AppSidebar.tsx:751` | FeatureGate 保護 |
| Direct messages | 私訊 | `AppSidebar.tsx:795` | SidebarSection title |
| Projects | 專案 | `SidebarProjectsSection.tsx:274` | SidebarProjectsSection title |
| Pinned | 置頂 | `sidebarData.ts:15` | 範例資料 |

### 3. 區段操作選單

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Mark all as read | 全部標記已讀 | `CustomChannelSection.tsx:190,192` | DropdownMenuItem |
| New message | 新增訊息 | `CustomChannelSection.tsx:196,198` | |
| Browse channels | 瀏覽頻道 | `CustomChannelSection.tsx:202,204` | |
| Create channel | 建立頻道 | `CustomChannelSection.tsx:210,213` | |
| Rename section | 重新命名區段 | `CustomChannelSection.tsx:219,223` | |
| Move up | 上移 | `CustomChannelSection.tsx:227,232` | |
| Move down | 下移 | `CustomChannelSection.tsx:236,241` | |
| Sort | 排序 | `CustomChannelSection.tsx:252` | Submenu 標題 |
| Recent | 最近活動 | `CustomChannelSection.tsx:73` | Sort option |
| A–Z | A–Z | `CustomChannelSection.tsx:74` | Sort option (保持英文) |
| Delete section | 刪除區段 | `CustomChannelSection.tsx:278,282` | |

### 4. 頻道右鍵選單

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Move to section | 移動至區段 | `ChannelContextMenu.tsx:62` | Submenu 標題 |
| New section... | 新增區段… | `ChannelContextMenu.tsx:91` | |
| Copy | 複製 | `ChannelContextMenu.tsx:117` | Submenu 標題 |
| Copy channel name | 複製頻道名稱 | `ChannelContextMenu.tsx:128` | |
| Copy channel ID | 複製頻道 ID | `ChannelContextMenu.tsx:135` | |
| Mark as read | 標記已讀 | `ChannelContextMenu.tsx:264` | |
| Mark unread | 標記未讀 | `ChannelContextMenu.tsx:275` | |
| Mute channel | 靜音頻道 | `ChannelContextMenu.tsx:298` | |
| Unmute channel | 取消靜音 | `ChannelContextMenu.tsx:289` | |
| Star channel | 加入星標 | `ChannelContextMenu.tsx:321` | |
| Unstar channel | 移除星標 | `ChannelContextMenu.tsx:312` | |
| Leave channel | 離開頻道 | `ChannelContextMenu.tsx:334` | |
| Archive channel | 封存頻道 | `ChannelContextMenu.tsx:361` | |
| Delete channel | 刪除頻道 | `ChannelContextMenu.tsx:373` | |
| Loading channel actions... | 載入頻道操作中… | `ChannelContextMenu.tsx:342` | |
| Channel actions unavailable | 頻道操作不可用 | `ChannelContextMenu.tsx:349` | |

### 5. 建立頻道/論壇對話框

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Create a new channel | 建立新頻道 | `CreateChannelDialog.tsx:73` | 標題 (channel) |
| Create a new forum | 建立新論壇 | `CreateChannelDialog.tsx:73` | 標題 (forum) |
| Channels are real-time streams for team conversation. | 頻道是團隊即時對話串流。 | `CreateChannelDialog.tsx:78` | 描述 (channel) |
| Forums organize threaded discussions around a topic. | 論壇以主題組織串式討論。 | `CreateChannelDialog.tsx:77` | 描述 (forum) |
| Name | 名稱 | `CreateChannelFormFields.tsx:74` | Label |
| design-discussions | 設計討論 | `CreateChannelFormFields.tsx:95` | Placeholder (forum) |
| release-notes | 版本資訊 | `CreateChannelFormFields.tsx:95` | Placeholder (channel) |
| Description | 說明 | `CreateChannelFormFields.tsx:109` | Label |
| Optional | 選填 | `CreateChannelFormFields.tsx:110` | |
| What this channel is for | 此頻道用途 | `CreateChannelFormFields.tsx:122` | Placeholder |
| What this forum is for | 此論壇用途 | `CreateChannelFormFields.tsx:122` | Placeholder |
| Type | 類型 | `CreateChannelFormFields.tsx:131` | Label |
| Template | 範本 | `CreateChannelFormFields.tsx:158` | Label |
| None | 無 | `CreateChannelFormFields.tsx:194` | 選項 |
| Create new channel template… | 建立新頻道範本… | `CreateChannelFormFields.tsx:205` | |
| Create channel | 建立頻道 | `CreateChannelFormFields.tsx:253` | 按鈕 |
| Create forum | 建立論壇 | `CreateChannelFormFields.tsx:253` | 按鈕 |
| Creating... | 建立中… | `CreateChannelFormFields.tsx:253` | 狀態 |

### 6. 新增社群對話框

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Add community | 新增社群 | `AddCommunityDialog.tsx:86` | 標題 (預設) |
| Create a new community | 建立新社群 | `AddCommunityDialog.tsx:82` | 標題 (建立模式) |
| Join an existing community | 加入現有社群 | `AddCommunityDialog.tsx:84` | 標題 (加入模式) |
| Opens Builderlab in your browser. | 將在瀏覽器開啟 Builderlab。 | `AddCommunityDialog.tsx:90` | 描述 (建立) |
| Use the community URL or invite link you received. | 使用您收到的社群網址或邀請連結。 | `AddCommunityDialog.tsx:92` | 描述 (加入) |
| Create a new community or join one you already have. | 建立新社群或加入現有社群。 | `AddCommunityDialog.tsx:93` | 描述 (預設) |
| Create a new community | 建立新社群 | `AddCommunityDialog.tsx:146` | 按鈕文字 |
| Claim a Buzz address for your team. | 為團隊申請 Buzz 位址。 | `AddCommunityDialog.tsx:149` | 說明文字 |
| Join an existing community | 加入現有社群 | `AddCommunityDialog.tsx:166` | 按鈕文字 |
| Use a community URL or invite link. | 使用社群網址或邀請連結。 | `AddCommunityDialog.tsx:169` | 說明文字 |

### 7. 區段建立/重命名/刪除對話框

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Create section | 建立區段 | `ChannelSectionDialogs.tsx:190` | 標題 |
| Sections let you group related channels in the sidebar. | 區段可讓您在側邊欄群組相關頻道。 | `ChannelSectionDialogs.tsx:191` | 描述 |
| Section name | 區段名稱 | `ChannelSectionDialogs.tsx:150` | Placeholder |
| Create | 建立 | `ChannelSectionDialogs.tsx:193` | 按鈕 |
| Rename section | 重新命名區段 | `ChannelSectionDialogs.tsx:219` | 標題 |
| Enter a new name for this section. | 輸入此區段的新名稱。 | `ChannelSectionDialogs.tsx:220` | 描述 |
| Save | 儲存 | `ChannelSectionDialogs.tsx:223` | 按鈕 |
| Delete section | 刪除區段 | `ChannelSectionDialogs.tsx:259` | AlertDialog 標題 |
| Delete section "X"? It has no channels. | 刪除區段「X」？該區段無頻道。 | `ChannelSectionDialogs.tsx:252` | 描述 (無頻道) |
| Delete section "X"? Its N channels will move back to the default Channels group. | 刪除區段「X」？其 N 個頻道將移回預設頻道群組。 | `ChannelSectionDialogs.tsx:253` | 描述 (有頻道) |
| Delete | 刪除 | `ChannelSectionDialogs.tsx:268` | 按鈕 |
| Cancel | 取消 | `ChannelSectionDialogs.tsx:159,263` | 按鈕 |

### 8. 離開頻道/刪除專案對話框

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Leave channel | 離開頻道 | `ChannelSectionDialogs.tsx:297` | 標題 |
| Leave "X"? You'll stop receiving its messages and can rejoin later. | 離開「X」？您將不再收到訊息，稍後可重新加入。 | `ChannelSectionDialogs.tsx:299` | 描述 |
| Leave | 離開 | `ChannelSectionDialogs.tsx:308` | 按鈕 |
| Delete project? | 刪除專案？ | `SidebarProjectsSection.tsx:409` | 標題 |
| Delete X from Projects for everyone. This can only be done for projects you own and cannot be undone. | 從所有人的專案中刪除 X。僅限專案擁有者執行，且無法復原。 | `SidebarProjectsSection.tsx:411` | 描述 |
| Delete project | 刪除專案 | `SidebarProjectsSection.tsx:442` | 按鈕 |
| Deleting... | 刪除中… | `SidebarProjectsSection.tsx:441` | 狀態 |

### 9. 專案側邊欄區段

| 英文原文 | 繁體中文建議 | 所在檔案 | 備註 |
|---------|-------------|---------|------|
| Projects | 專案 | `SidebarProjectsSection.tsx:274` | 區段標題 |
| Add project | 新增專案 | `SidebarProjectsSection.tsx:485` | Tooltip |
| More actions for Projects | 專案更多操作 | `SidebarProjectsSection.tsx:493` | Tooltip |
| Show | 顯示 | `SidebarProjectsSection.tsx:517` | Submenu 標題 |
| Added | 已新增 | `SidebarProject
