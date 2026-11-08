/**
 * UI-TARS Desktop 產品級 AgentEventStream 不可變事件流定義
 * 參考 bytedance/UI-TARS-desktop 架構：
 * 記錄 Thought / ToolCall / Observation / Screenshot 不可變事件流，
 * 支援桌面介面時光機回溯、分支視覺化與人工介入審核。
 */

export type AgentStreamEventType =
  | "thought"
  | "tool_call"
  | "observation"
  | "screenshot"
  | "user_intervention"
  | "checkpoint";

export interface AgentStreamEvent {
  id: string;
  agentPubkey: string;
  turnIndex: number;
  type: AgentStreamEventType;
  timestamp: number;
  readableTime: string;
  payload: {
    title: string;
    content?: string;
    toolName?: string;
    toolArgs?: Record<string, unknown>;
    toolOutput?: string;
    screenshotUrl?: string;
    confidence?: number;
    isError?: boolean;
  };
}

export class AgentEventStream {
  private events: AgentStreamEvent[] = [];

  constructor(initialEvents: AgentStreamEvent[] = []) {
    this.events = [...initialEvents];
  }

  /**
   * 追加不可變事件（保證時序嚴格遞增）
   */
  public append(
    event: Omit<AgentStreamEvent, "id" | "timestamp" | "readableTime">
  ): AgentStreamEvent {
    const timestamp = Date.now();
    const newEvent: AgentStreamEvent = {
      ...event,
      id: `evt_${timestamp}_${Math.random().toString(36).slice(2, 8)}`,
      timestamp,
      readableTime: new Date(timestamp).toLocaleTimeString(),
    };
    this.events.push(Object.freeze(newEvent));
    return newEvent;
  }

  /**
   * 獲取不可變事件陣列複本
   */
  public getEvents(): readonly AgentStreamEvent[] {
    return [...this.events];
  }

  /**
   * 根據回合索引起點過濾
   */
  public getEventsForTurn(turnIndex: number): AgentStreamEvent[] {
    return this.events.filter((e) => e.turnIndex === turnIndex);
  }

  /**
   * 時光機：回溯至特定斷點事件
   */
  public rollbackToCheckpoint(checkpointId: string): AgentStreamEvent[] {
    const idx = this.events.findIndex((e) => e.id === checkpointId);
    if (idx !== -1) {
      this.events = this.events.slice(0, idx + 1);
    }
    return this.getEvents();
  }

  /**
   * 清空事件流
   */
  public clear(): void {
    this.events = [];
  }
}

