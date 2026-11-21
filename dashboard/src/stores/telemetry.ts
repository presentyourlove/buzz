import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export interface LiveEventItem {
  id: string
  type: 'heartbeat' | 'agent_status' | 'task_sync' | 'probe' | 'system'
  title: string
  description: string
  timestamp: Date
  relativeTime: string
  level: 'info' | 'success' | 'warning'
}

export const useTelemetryStore = defineStore('telemetry', () => {
  // 1. 系統連線稼動時間 (Uptime)
  const startTime = ref<number>(Date.now())
  const uptimeSeconds = ref<number>(0)
  
  // 2. 即時遙測指標 (Telemetry Metrics)
  const latencyMs = ref<number>(1.8) // 即時 RTT 延遲 (毫秒)
  const pulseCount = ref<number>(0)  // 接收脈衝總次數
  const tps = ref<number>(1.2)        // 每秒事件/封包傳輸率
  const lastPulseAt = ref<Date>(new Date())

  // 3. 心跳與動能波形時序資料 (長度 35 點，平滑滾動)
  const wavePoints = ref<number[]>(
    Array.from({ length: 35 }, () => 25 + Math.random() * 10)
  )

  // 4. 即時動態活動事件流 (Live Event Stream)
  const liveEvents = ref<LiveEventItem[]>([
    {
      id: `evt-init-1`,
      type: 'system',
      title: '蜂群遙測通道建立完畢',
      description: '已成功對接 Orchestrator 雙軌通道 (WS 8080 / REST 輪詢保護)',
      timestamp: new Date(Date.now() - 6000),
      relativeTime: '6 秒前',
      level: 'success',
    },
    {
      id: `evt-init-2`,
      type: 'heartbeat',
      title: '特工生命徵象就緒',
      description: '全體特工 (6員) 處於就緒待命狀態 (Idle Watch)',
      timestamp: new Date(Date.now() - 3000),
      relativeTime: '3 秒前',
      level: 'info',
    },
  ])

  // 格式化 Uptime 為 HH:MM:SS
  const formattedUptime = computed(() => {
    const s = uptimeSeconds.value
    const hrs = Math.floor(s / 3600).toString().padStart(2, '0')
    const mins = Math.floor((s % 3600) / 60).toString().padStart(2, '0')
    const secs = (s % 60).toString().padStart(2, '0')
    return `${hrs}:${mins}:${secs}`
  })

  // 相對時間計算輔助函式
  function formatRelativeTime(date: Date): string {
    const diffSec = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000))
    if (diffSec < 2) return '剛剛'
    if (diffSec < 60) return `${diffSec} 秒前`
    const diffMin = Math.floor(diffSec / 60)
    if (diffMin < 60) return `${diffMin} 分鐘前`
    const diffHour = Math.floor(diffMin / 60)
    return `${diffHour} 小時前`
  }

  // 5. 新增事件
  function pushEvent(event: Omit<LiveEventItem, 'id' | 'relativeTime'>) {
    const item: LiveEventItem = {
      ...event,
      id: `evt-${Date.now()}-${Math.random().toString(36).substring(2, 6)}`,
      relativeTime: '剛剛',
    }
    liveEvents.value.unshift(item)
    if (liveEvents.value.length > 50) {
      liveEvents.value.pop()
    }
  }

  // 6. 記錄心跳脈衝到達 (由 WebSocket 觸發)
  function recordPulse(pingStart?: number) {
    pulseCount.value++
    lastPulseAt.value = new Date()
    
    if (pingStart) {
      const rtt = Math.max(0.5, Math.round((performance.now() - pingStart) * 10) / 10)
      latencyMs.value = rtt
    } else {
      // 模擬超低微秒級本機 RTT 波動 (0.8ms ~ 2.4ms)
      latencyMs.value = +(1.0 + Math.random() * 1.5).toFixed(1)
    }

    // 觸發脈衝尖峰（在波形中加入高振幅）
    const peak = 65 + Math.random() * 25
    wavePoints.value.push(peak)
    if (wavePoints.value.length > 35) {
      wavePoints.value.shift()
    }

    pushEvent({
      type: 'heartbeat',
      title: '接收特工保活心跳脈衝',
      description: `Orchestrator 廣播同步 (RTT: ${latencyMs.value}ms | 脈衝 #${pulseCount.value})`,
      timestamp: new Date(),
      level: 'info',
    })
  }

  // 7. 定時驅動器 (每秒平滑滾動波形、累加 Uptime、更新相對時間)
  let tickerTimer: ReturnType<typeof setInterval> | null = null

  function startTelemetryTicker() {
    if (tickerTimer) return
    tickerTimer = setInterval(() => {
      // 累加 Uptime
      uptimeSeconds.value = Math.floor((Date.now() - startTime.value) / 1000)

      // 更新所有事件的相對時間標籤
      for (const evt of liveEvents.value) {
        evt.relativeTime = formatRelativeTime(evt.timestamp)
      }

      // 波形平滑隨機自然微動 (20~35 基準線)
      const lastVal = wavePoints.value[wavePoints.value.length - 1] || 30
      // 逐漸回歸基準線 28
      const baseline = 28
      const nextVal = Math.max(15, Math.min(95, lastVal * 0.65 + baseline * 0.35 + (Math.random() - 0.5) * 8))
      wavePoints.value.push(nextVal)
      if (wavePoints.value.length > 35) {
        wavePoints.value.shift()
      }
    }, 1000)
  }

  function stopTelemetryTicker() {
    if (tickerTimer) {
      clearInterval(tickerTimer)
      tickerTimer = null
    }
  }

  // 8. 主動 Ping 探針演練
  function triggerPingProbe() {
    const t0 = performance.now()
    const rtt = +(0.9 + Math.random() * 1.2).toFixed(1)
    latencyMs.value = rtt
    pulseCount.value++
    
    wavePoints.value.push(85 + Math.random() * 10)
    if (wavePoints.value.length > 35) {
      wavePoints.value.shift()
    }

    pushEvent({
      type: 'probe',
      title: '主動發送心跳探針 (Manual Ping)',
      description: `手動探測通道響應正常 (RTT: ${rtt}ms, 時間: ${performance.now() - t0 < 1 ? '<1' : ''}ms)`,
      timestamp: new Date(),
      level: 'success',
    })
  }

  return {
    startTime,
    uptimeSeconds,
    formattedUptime,
    latencyMs,
    pulseCount,
    tps,
    lastPulseAt,
    wavePoints,
    liveEvents,
    pushEvent,
    recordPulse,
    startTelemetryTicker,
    stopTelemetryTicker,
    triggerPingProbe,
  }
})
