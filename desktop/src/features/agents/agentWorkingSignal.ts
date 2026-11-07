import * as React from "react";

import { normalizePubkey } from "@/shared/lib/pubkey";
import {
  type ActiveChannelTurnSummary,
  getActiveTurnsByChannel,
  getActiveTurnsForAgent,
  subscribeActiveAgentTurns,
} from "./activeAgentTurnsStore";

/**
 * Unified "agent is working" signal.
 *
 * Every surface that shows a working affordance (sidebar channel badges,
 * profile badges, agent rows, composer activity bar, activity panel header,
 * future thread ingresses) should read from this module instead of picking
 * one of the underlying pipes. The rule is:
 *
 *   1. Observer-derived active turns (kind 24200 → activeAgentTurnsStore)
 *      are the primary signal — they carry channel scope and a start anchor.
 *   2. Bot typing indicators (kind 20002, mirrored into this module by the
 *      channel typing hooks) are the fallback for agents whose observer
 *      stream is absent for that scope (e.g. remote harness without relay
 *      observer, or frames not yet arrived).
 *
 * Scope rule: with a channelId, "working" means working in that channel;
 * without one, "working" means any active work in any channel (the
 * all-channels rule the activity panel uses).
 */

export type AgentWorkingSource = "observer" | "typing" | "none";

export type AgentWorkingChannel = {
  channelId: string;
  /** Desktop-clock anchor for elapsed displays (turn start / first typing). */
  anchorAt: number;
  source: Exclude<AgentWorkingSource, "none">;
};

export type AgentWorkingState = {
  working: boolean;
  /** Strongest signal backing `working` for the requested scope. */
  source: AgentWorkingSource;
  /** Every channel the agent is working in (unscoped), observer-primary. */
  channels: AgentWorkingChannel[];
};

export type WorkingChannelSummary = ActiveChannelTurnSummary & {
  source: Exclude<AgentWorkingSource, "none">;
};

const IDLE_STATE: AgentWorkingState = {
  working: false,
  source: "none",
  channels: [],
};

// ── Typing registry (fallback input) ────────────────────────────────────────
// channelId → (normalized agent pubkey → first-seen ms). Fed by
// reportChannelBotTyping from the channel typing hooks; entries follow the
// typing TTL because the hooks re-report whenever their entries change.
const typingByChannel = new Map<string, Map<string, number>>();

const listeners = new Set<() => void>();

// localStorage persistence for typingByChannel (Mode B: Phase 2)
const STORAGE_KEY = 'buzz:agent-working-signal';
const STORAGE_VERSION = 1;

interface PersistedTypingEntry {
  pubkey: string;
  anchorAt: number;
}

interface PersistedTypingState {
  version: number;
  updatedAt: number;
  typingByChannel: Record<string, PersistedTypingEntry[]>;
}

function loadPersistedTyping(): Map<string, Map<string, number>> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return new Map();
    const parsed: PersistedTypingState = JSON.parse(raw);
    if (parsed.version !== STORAGE_VERSION) return new Map();
    const map = new Map<string, Map<string, number>>();
    for (const [channelId, entries] of Object.entries(parsed.typingByChannel)) {
      if (Date.now() - parsed.updatedAt < 10 * 60 * 1000) {
        const channelMap = new Map<string, number>();
        for (const entry of entries) {
          channelMap.set(entry.pubkey, entry.anchorAt);
        }
        map.set(channelId, channelMap);
      }
    }
    return map;
  } catch {
    return new Map();
  }
}

let persistDebounce: ReturnType<typeof setTimeout> | null = null;
function schedulePersist(typingMap: Map<string, Map<string, number>>) {
  if (persistDebounce) clearTimeout(persistDebounce);
  persistDebounce = setTimeout(() => {
    const typingByChannel: Record<string, PersistedTypingEntry[]> = {};
    for (const [channelId, pubkeys] of typingMap.entries()) {
      typingByChannel[channelId] = Array.from(pubkeys.entries()).map(([pubkey, anchorAt]) => ({
        pubkey,
        anchorAt,
      }));
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      version: STORAGE_VERSION,
      updatedAt: Date.now(),
      typingByChannel,
    }));
  }, 500);
}

const initialTyping = loadPersistedTyping();
for (const [channelId, pubkeys] of initialTyping.entries()) {
  typingByChannel.set(channelId, pubkeys);
}

let unsubscribeTurns: (() => void) | null = null;

// Reference-stable snapshots for useSyncExternalStore. React reads a snapshot
// before it subscribes, so these must be stable even with no listeners yet.
const stateCache = new Map<string, AgentWorkingState>();
let channelsCache: WorkingChannelSummary[] | null = null;
const channelPubkeysCache = new Map<string, string[]>();

function invalidateCaches() {
  stateCache.clear();
  channelsCache = null;
  channelPubkeysCache.clear();
}

function notify() {
  invalidateCaches();
  for (const listener of listeners) {
    listener();
  }
}

export function subscribeAgentWorkingSignal(listener: () => void) {
  listeners.add(listener);
  if (listeners.size === 1) {
    invalidateCaches();
    unsubscribeTurns = subscribeActiveAgentTurns(notify);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      unsubscribeTurns?.();
      unsubscribeTurns = null;
    }
  };
}

/**
 * Mirror the current bot typing pubkeys for a channel into the signal.
 * Call with the full current set (empty array clears the channel). First-seen
 * timestamps are preserved across re-reports so elapsed anchors stay stable.
 */
export function reportChannelBotTyping(
  channelId: string,
  pubkeys: readonly string[],
) {
  const current = typingByChannel.get(channelId);
  const next = new Map<string, number>();
  const now = Date.now();
  for (const pubkey of pubkeys) {
    const key = normalizePubkey(pubkey);
    next.set(key, current?.get(key) ?? now);
  }

  const unchanged =
    (current?.size ?? 0) === next.size &&
    [...next.keys()].every((key) => current?.has(key));
  if (unchanged) {
    return;
  }

  if (next.size === 0) {
    typingByChannel.delete(channelId);
  } else {
    typingByChannel.set(channelId, next);
  }

  // 持久化到 localStorage (去抖動)
  schedulePersist(typingByChannel);

  // Queue for batched broadcast (Phase 3: 500ms debounce)
  if (next.size > 0) {
    const earliestAnchor = Math.min(...next.values());
    queueAgentStatusBroadcast(channelId, [...next.keys()], earliestAnchor, "typing");
  }

  notify();
}

function computeAgentWorkingState(
  agentPubkey: string,
  channelId: string | null,
): AgentWorkingState {
  const key = normalizePubkey(agentPubkey);
  const turns = getActiveTurnsForAgent(key);

  const channels: AgentWorkingChannel[] = turns.map((turn) => ({
    channelId: turn.channelId,
    anchorAt: turn.anchorAt,
    source: "observer" as const,
  }));
  const observerChannelIds = new Set(turns.map((turn) => turn.channelId));

  for (const [typingChannelId, entries] of typingByChannel) {
    if (observerChannelIds.has(typingChannelId)) {
      continue;
    }
    const since = entries.get(key);
    if (since !== undefined) {
      channels.push({
        channelId: typingChannelId,
        anchorAt: since,
        source: "typing",
      });
    }
  }

  if (channels.length === 0) {
    return IDLE_STATE;
  }

  channels.sort((a, b) => a.channelId.localeCompare(b.channelId));

  const scoped =
    channelId === null
      ? channels
      : channels.filter((channel) => channel.channelId === channelId);
  const source: AgentWorkingSource = scoped.some(
    (channel) => channel.source === "observer",
  )
    ? "observer"
    : scoped.length > 0
      ? "typing"
      : "none";

  return { working: source !== "none", source, channels };
}

/**
 * Working state for one agent, optionally scoped to a channel. Returns a
 * reference-stable snapshot while subscribed (useSyncExternalStore-safe).
 */
export function getAgentWorkingState(
  agentPubkey: string | null | undefined,
  channelId: string | null = null,
): AgentWorkingState {
  if (!agentPubkey) {
    return IDLE_STATE;
  }
  const cacheKey = `${normalizePubkey(agentPubkey)}|${channelId ?? ""}`;
  const cached = stateCache.get(cacheKey);
  if (cached) {
    return cached;
  }
  const state = computeAgentWorkingState(agentPubkey, channelId);
  stateCache.set(cacheKey, state);
  return state;
}

/**
 * All channels with agent work in progress, aggregated across agents and
 * merged observer-primary: typing-only agents fold into an existing observer
 * summary; channels with only typing get a typing-sourced summary anchored to
 * first-seen typing.
 */
export function getWorkingChannels(): WorkingChannelSummary[] {
  if (channelsCache) {
    return channelsCache;
  }

  const byChannel = new Map<string, WorkingChannelSummary>();
  for (const summary of getActiveTurnsByChannel()) {
    byChannel.set(summary.channelId, { ...summary, source: "observer" });
  }

  for (const [channelId, entries] of typingByChannel) {
    const existing = byChannel.get(channelId);
    if (existing) {
      const known = new Set(
        existing.agentPubkeys.map((pubkey) => normalizePubkey(pubkey)),
      );
      const merged = [...existing.agentPubkeys];
      for (const pubkey of entries.keys()) {
        if (!known.has(pubkey)) {
          merged.push(pubkey);
        }
      }
      if (merged.length !== existing.agentPubkeys.length) {
        byChannel.set(channelId, {
          ...existing,
          agentPubkeys: merged,
          agentCount: merged.length,
        });
      }
      continue;
    }

    let anchorAt = Number.POSITIVE_INFINITY;
    for (const since of entries.values()) {
      if (since < anchorAt) {
        anchorAt = since;
      }
    }
    byChannel.set(channelId, {
      channelId,
      anchorAt,
      agentCount: entries.size,
      agentPubkeys: [...entries.keys()],
      source: "typing",
    });
  }

  const result = [...byChannel.values()].sort((a, b) =>
    a.channelId.localeCompare(b.channelId),
  );
  channelsCache = result;
  return result;
}

const EMPTY_PUBKEYS: string[] = [];

/**
 * Normalized pubkeys of every agent working in the given channel
 * (observer turns ∪ typing fallback). Stable while subscribed.
 */
export function getWorkingAgentPubkeysForChannel(
  channelId: string | null | undefined,
): string[] {
  if (!channelId) {
    return EMPTY_PUBKEYS;
  }
  const cached = channelPubkeysCache.get(channelId);
  if (cached) {
    return cached;
  }
  const merged = new Set<string>();
  for (const summary of getActiveTurnsByChannel()) {
    if (summary.channelId !== channelId) {
      continue;
    }
    for (const pubkey of summary.agentPubkeys) {
      merged.add(normalizePubkey(pubkey));
    }
  }
  const typing = typingByChannel.get(channelId);
  if (typing) {
    for (const pubkey of typing.keys()) {
      merged.add(pubkey);
    }
  }
  const result = merged.size === 0 ? EMPTY_PUBKEYS : [...merged].sort();
  channelPubkeysCache.set(channelId, result);
  return result;
}

// ── Hooks ────────────────────────────────────────────────────────────────────

/** Working state for one agent, optionally scoped to a channel. */
export function useAgentWorking(
  agentPubkey: string | null | undefined,
  channelId: string | null = null,
): AgentWorkingState {
  return React.useSyncExternalStore(subscribeAgentWorkingSignal, () =>
    getAgentWorkingState(agentPubkey, channelId),
  );
}

/** All channels with agent work in progress, across agents. */
export function useWorkingChannels(): WorkingChannelSummary[] {
  return React.useSyncExternalStore(
    subscribeAgentWorkingSignal,
    getWorkingChannels,
  );
}

/** Normalized pubkeys of agents working in a channel. */
export function useChannelWorkingAgentPubkeys(
  channelId: string | null | undefined,
): string[] {
  return React.useSyncExternalStore(subscribeAgentWorkingSignal, () =>
    getWorkingAgentPubkeysForChannel(channelId),
  );
}

/** Community-switch reset (see resetCommunityState in useCommunityInit). */
export function resetAgentWorkingSignal() {
  typingByChannel.clear();
  invalidateCaches();
  for (const listener of listeners) {
    listener();
  }
}

// ─── Phase 3: 500ms Debounce + Batch Broadcast ───────────────────────────────
/**
 * Batched agent working status broadcaster.
 * 
 * Collects agent status changes within a 500ms window and publishes them
 * as a single batched event to reduce Relay congestion.
 * 
 * Based on the keepalive ticker pattern in agent.rs:420-439.
 */

interface BatchedStatusUpdate {
  channelId: string;
  agentPubkeys: string[];
  anchorAt: number;
  source: Exclude<AgentWorkingSource, "none">;
}

const DEBOUNCE_MS = 500;
let broadcastQueue: BatchedStatusUpdate[] = [];
let broadcastTimeout: ReturnType<typeof setTimeout> | null = null;
let broadcastRelayUrl: string | null = null;
let broadcastKeys: { secretKey: string; publicKey: string } | null = null;

/**
 * Configure the batched broadcaster with relay connection details.
 * Call this once at app startup.
 */
export function configureBatchedBroadcast(
  relayUrl: string,
  keys: { secretKey: string; publicKey: string }
) {
  broadcastRelayUrl = relayUrl;
  broadcastKeys = keys;
}

/**
 * Queue an agent working status update for batched broadcast.
 * Multiple updates within DEBOUNCE_MS are merged into a single event.
 */
export function queueAgentStatusBroadcast(
  channelId: string,
  agentPubkeys: string[],
  anchorAt: number,
  source: Exclude<AgentWorkingSource, "none">,
) {
  if (!broadcastRelayUrl || !broadcastKeys) {
    // Not configured - skip broadcast (graceful fallback)
    return;
  }

  // Normalize pubkeys
  const normalized = agentPubkeys.map(normalizePubkey).sort();

  // Check if we already have a pending update for this channel
  const existingIdx = broadcastQueue.findIndex(
    (u) => u.channelId === channelId
  );

  if (existingIdx >= 0) {
    // Merge with existing - union of pubkeys, earliest anchor
    const existing = broadcastQueue[existingIdx];
    const mergedPubkeys = [...new Set([...existing.agentPubkeys, ...normalized])].sort();
    broadcastQueue[existingIdx] = {
      channelId,
      agentPubkeys: mergedPubkeys,
      anchorAt: Math.min(existing.anchorAt, anchorAt),
      source: existing.source === "observer" || source === "observer" ? "observer" : "typing",
    };
  } else {
    broadcastQueue.push({ channelId, agentPubkeys: normalized, anchorAt, source });
  }

  // Schedule flush if not already scheduled
  if (broadcastTimeout === null) {
    broadcastTimeout = setTimeout(flushBroadcastQueue, DEBOUNCE_MS);
  }
}

/**
 * Flush the broadcast queue - publishes all pending updates as a batched event.
 * Uses Nostr WebSocket connection (same as daemon_ws.ts pattern).
 */
async function flushBroadcastQueue() {
  broadcastTimeout = null;
  if (broadcastQueue.length === 0 || !broadcastRelayUrl || !broadcastKeys) {
    return;
  }

  const updates = [...broadcastQueue];
  broadcastQueue = [];

  try {
    // Build batched event content
    const eventContent = JSON.stringify({
      type: "agent_working_batch",
      updates: updates.map((u) => ({
        channelId: u.channelId,
        agentPubkeys: u.agentPubkeys,
        anchorAt: u.anchorAt,
        source: u.source,
      })),
      timestamp: Date.now(),
    });

    // Create and sign event using nostr-tools (Kind 24200 - Agent Working Status)
    const { EventBuilder, Keys, Kind, Tag, finalizeEvent } = await import("nostr-tools");
    const keys = Keys.parse(broadcastKeys.secretKey);
    
    // Build unsigned event
    const unsignedEvent = EventBuilder.new(Kind.Custom(24200), eventContent)
      .tag(new Tag(["batch", "agent_working_status"]))
      .build();
    
    // Sign the event
    const event = finalizeEvent(unsignedEvent, keys.privateKey);
    
    // Publish via WebSocket (Nostr Relay protocol) - same as daemon_ws.rs pattern
    await publishEventViaWebSocket(broadcastRelayUrl, event);

  } catch (error) {
    console.error("[agentWorkingSignal] Batched broadcast error:", error);
    // Re-queue for retry
    broadcastQueue.unshift(...updates);
    if (broadcastQueue.length <= 50) {
      broadcastTimeout = setTimeout(flushBroadcastQueue, DEBOUNCE_MS * 2);
    }
  }
}

/**
 * Publish a Nostr event via WebSocket connection to Relay.
 * Mirrors the pattern in daemon_ws.rs:148-155.
 */
async function publishEventViaWebSocket(relayUrl: string, event: any): Promise<void> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relayUrl);
    let resolved = false;
    
    const timeout = setTimeout(() => {
      if (!resolved) {
        resolved = true;
        ws.close();
        reject(new Error("WebSocket connection timeout"));
      }
    }, 5000);
    
    ws.onopen = () => {
      // Send EVENT message per Nostr protocol
      ws.send(JSON.stringify(["EVENT", event]));
    };
    
    ws.onmessage = (msg) => {
      try {
        const data = JSON.parse(msg.data);
        if (data[0] === "OK" && data[1] === event.id) {
          if (data[2] === true) {
            if (!resolved) {
              resolved = true;
              clearTimeout(timeout);
              ws.close();
              resolve();
            }
          } else {
            if (!resolved) {
              resolved = true;
              clearTimeout(timeout);
              ws.close();
              reject(new Error(`Relay rejected event: ${data[3] || "unknown reason"}`));
            }
          }
        }
      } catch {
        // Ignore parse errors
      }
    };
    
    ws.onerror = (err) => {
      if (!resolved) {
        resolved = true;
        clearTimeout(timeout);
        ws.close();
        reject(err);
      }
    };
    
    ws.onclose = () => {
      if (!resolved) {
        resolved = true;
        clearTimeout(timeout);
        reject(new Error("WebSocket closed unexpectedly"));
      }
    };
  });
}

/**
 * Force flush any pending broadcasts immediately (e.g., on page unload).
 */
export function flushPendingBroadcasts() {
  if (broadcastTimeout) {
    clearTimeout(broadcastTimeout);
    broadcastTimeout = null;
  }
  if (broadcastQueue.length > 0) {
    // Fire and forget - best effort on unload
    flushBroadcastQueue();
  }
}
/**
 * Clean up all timers and intervals (for testing, hot-reload, or unload).
 */
export function cleanupAgentWorkingSignal(): void {
  if (persistDebounce) {
    clearTimeout(persistDebounce);
    persistDebounce = null;
  }
  if (broadcastTimeout) {
    clearTimeout(broadcastTimeout);
    broadcastTimeout = null;
  }
  broadcastQueue = [];
}
