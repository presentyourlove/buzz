---
name: code-defined-narrative
description: 基於程式碼定義的動態敘事與影片分鏡生成引擎 (借鑒 claude-animated-shorts Remotion + Three.js 架構)
version: 1.0.0
tags: [narrative, remotion, threejs, animation, multi-agent]
---

# 🎬 程式碼定義動態敘事協作協議 (Code-Defined Narrative Engine Protocol)

> **架構借鑒**：[mromidev/claude-animated-shorts](https://github.com/mromidev/claude-animated-shorts) (★2630)  
> **核心哲學**：**「程式碼即媒介 (Code as Media)」**。拒絕不可控、高成本的雲端 Diffusion 生成，採用確定性、低延遲的本機 Remotion (React) + Three.js (3D 空間鏡頭) + ffmpeg 進行高精準度短影音創作。

---

## 一、 多特工協作工作流 (Multi-Agent Content Pipeline)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. 編劇特工 (Honey2)                                                        │
│    - 負責劇本構思、情節起伏設計、語音旁白 (Voiceover Script) 與時長規劃        │
│    - 產出: story_draft.md                                                   │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 2. 分鏡與代碼特工 (Pollen)                                                  │
│    - 將劇本轉譯為強型別 TypeScript 結構化分鏡表 (Structured Shot Plan)      │
│    - 規劃 Three.js 空間座標、2D 角色立繪層、運鏡路徑 (Gliding Cameras)      │
│    - 產出: story.ts (Remotion 渲染契約)                                     │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 3. 渲染與安全特工 (Sentry)                                                  │
│    - 語法 AST 靜態檢驗，防範不安全代碼與內存溢出                             │
│    - 啟動本地無頭渲染 (Headless Remotion Bundle + ffmpeg 轉碼)               │
│    - 產出: output.mp4 並推送通知至討論串                                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 二、 結構化分鏡資料結構契約 (`story.ts` Schema)

所有參與分鏡生成的特工必須嚴格遵循以下 TypeScript 結構定義：

```typescript
export interface NarrativeScene {
  id: string;
  durationInFrames: number;      // 30 fps 基底
  camera: {
    position: [number, number, number]; // [x, y, z] 3D 空間視角
    target: [number, number, number];   // 注視點
    fov: number;
    glidingPath?: "linear" | "smooth_orbit" | "zoom_in";
  };
  character: {
    name: string;
    avatarAsset: string;         // 2D 角色立繪圖檔路徑
    position: [number, number];  // 2D 螢幕投影定位
    animation: "idle_bounce" | "enter_left" | "nod_talk" | "shock_shake";
  };
  dialogue: {
    speaker: string;
    subtitleText: string;
    audioVoiceFile?: string;
  };
  background3D: {
    theme: "cyber_grid" | "clean_office" | "cosmic_stars" | "abstract_flow";
    primaryColor: string;
  };
}

export interface StoryBoard {
  title: string;
  fps: 30;
  totalDurationSeconds: number;
  scenes: NarrativeScene[];
}
```

---

## 三、 本地渲染執行命令規範

特工透過本機命令行直接喚起無頭渲染管線：

```bash
# 1. 執行分鏡語法檢驗 (Dry-run AST Validate)
node tools/narrative_render_bridge.mjs --validate story.ts

# 2. 本地快速預覽合成 (HQ mp4 產出)
node tools/narrative_render_bridge.mjs --render story.ts --output outbox/short_narrative.mp4
```

---

## 四、 核心安全守則 (Sentry Gatekeeper Rules)

1. **嚴禁雲端 Token 外洩**：渲染過程 100% 本地進行，不依賴任何外部商業影片渲染 API。
2. **防爆幀率控制**：單一場景幀數不可超過 1800 幀（60 秒），防止記憶體溢出。
3. **路徑安全沙盒**：所有引用的貼圖、音效素材僅允許讀取專案內部資產目錄，嚴禁跨目錄讀取系統敏感檔案。

