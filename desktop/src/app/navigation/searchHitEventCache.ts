import type { RelayEvent, SearchHit } from "@/shared/api/types";
import { openDB, DBSchema, IDBPDatabase } from 'idb';

interface SearchHitCacheDB extends DBSchema {
  searchHits: {
    key: string;
    value: {
      event: RelayEvent;
      channelId: string;
      messageId: string;
      cachedAt: number;
      ttl: number;
    };
    indexes: { 'by-channel': string; 'by-expires': number };
  };
}

const DB_NAME = 'buzz-search-hit-cache';
const DB_VERSION = 1;
const DEFAULT_TTL_MS = 24 * 60 * 60 * 1000; // 24h
const MAX_CACHED_EVENTS = 200;

let dbPromise: Promise<IDBPDatabase<SearchHitCacheDB>> | null = null;

function getDB() {
  if (!dbPromise) {
    dbPromise = openDB<SearchHitCacheDB>(DB_NAME, DB_VERSION, {
      upgrade(db) {
        const store = db.createObjectStore('searchHits', { keyPath: 'key' });
        store.createIndex('by-channel', 'channelId');
        store.createIndex('by-expires', 'cachedAt');
      },
    });
  }
  return dbPromise;
}

export async function cacheSearchHitEvent(hit: SearchHit): Promise<RelayEvent> {
  const db = await getDB();
  const event = buildSearchHitEvent(hit);
  const key = `${hit.channelId}:${event.id}`;
  await db.put('searchHits', {
    key,
    event,
    channelId: hit.channelId,
    messageId: event.id,
    cachedAt: Date.now(),
    ttl: DEFAULT_TTL_MS,
  });
  return event;
}

export async function getCachedSearchHitEvent(
  targetMessageId: string,
  channelId?: string
): Promise<RelayEvent | null> {
  const db = await getDB();
  
  if (channelId) {
    // 精確查詢
    const key = `${channelId}:${targetMessageId}`;
    const entry = await db.get('searchHits', key);
    if (entry && !isExpired(entry)) return entry.event;
    return null;
  }
  
  // 跨頻道查詢：全表掃描（無 channelId 時）
  const entries = await db.getAll('searchHits');
  for (const entry of entries) {
    if (entry.messageId === targetMessageId && !isExpired(entry)) {
      return entry.event;
    }
  }
  return null;
}

function isExpired(entry: SearchHitCacheDB['searchHits']['value']): boolean {
  return Date.now() - entry.cachedAt > entry.ttl;
}

export function buildSearchHitEvent(hit: SearchHit): RelayEvent {
  return {
    id: hit.eventId,
    pubkey: hit.pubkey,
    created_at: hit.createdAt,
    kind: hit.kind,
    tags: hit.channelId ? [["h", hit.channelId]] : [],
    content: hit.content,
    sig: "",
  };
}

export async function clearSearchHitEventCache(): Promise<void> {
  const db = await getDB();
  await db.clear('searchHits');
}

// 定期清理過期條目 (每小時) - 返回清理函數以便銷毀
let cleanupIntervalId: ReturnType<typeof setInterval> | null = null;

export function startCleanupInterval(): () => void {
  if (cleanupIntervalId !== null) {
    return () => stopCleanupInterval();
  }
  cleanupIntervalId = setInterval(async () => {
    const db = await getDB();
    const now = Date.now();
    const tx = db.transaction('searchHits', 'readwrite');
    const index = tx.store.index('by-expires');
    let cursor = await index.openCursor();
    while (cursor) {
      if (now - cursor.value.cachedAt > cursor.value.ttl) {
        await cursor.delete();
      }
      cursor = await cursor.continue();
    }
    await tx.done;
  }, 60 * 60 * 1000);
  return () => stopCleanupInterval();
}

export function stopCleanupInterval(): void {
  if (cleanupIntervalId !== null) {
    clearInterval(cleanupIntervalId);
    cleanupIntervalId = null;
  }
}
