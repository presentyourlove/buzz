import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Agent, AgentStatus, WSMessage } from '@/types'
import { useApi } from '@/composables'
import { useTelemetryStore } from './telemetry'

// 預設蜂群 6 大特工真實名冊 (離線韌性架構)
const BUZZ_TEAM_AGENTS: Agent[] = [
  {
    id: 'fizz',
    name: 'Fizz',
    displayName: 'Fizz (秘書)',
    role: '秘書處總管 / 任務派單與跨特工分流',
    status: 'idle',
    lastActiveAt: new Date().toISOString(),
    capabilities: ['coordination', 'task-dispatch', 'communication'],
    metadata: { department: '秘書處', model: 'gemini-3.1-flash-lite' },
  },
  {
    id: 'honey',
    name: 'Honey',
    displayName: 'Honey (大腦)',
    role: '資料研究部 / 高頻代碼檢索與前沿情報',
    status: 'idle',
    lastActiveAt: new Date().toISOString(),
    capabilities: ['code-search', 'research', 'fact-check'],
    metadata: { department: '資料研究部', model: 'gemini-3.5-flash-lite' },
  },
  {
    id: 'honey2',
    name: 'Honey2',
    displayName: 'Honey2 (架構企劃官)',
    role: '技術開發部 / 儀表板架構設計與開源雷達',
    status: 'idle',
    lastActiveAt: new Date().toISOString(),
    capabilities: ['frontend', 'architecture', 'vue3', 'tailwind'],
    metadata: { department: '技術開發部', model: 'nemotron-3-ultra-550b-a55b:free' },
  },
  {
    id: 'pollen',
    name: 'Pollen',
    displayName: 'Pollen (技術架構師)',
    role: '技術開發部 / 核心代碼實作與 Patch 補丁',
    status: 'idle',
    lastActiveAt: new Date().toISOString(),
    capabilities: ['rust', 'backend', 'gates', 'unit-tests'],
    metadata: { department: '技術開發部', model: 'nemotron-3-ultra-550b-a55b' },
  },
  {
    id: 'sentry',
    name: 'Sentry',
    displayName: 'Sentry (品質與資安總監)',
    role: '品質監控部 / Triple Gate 門禁與 QA 簽核',
    status: 'idle',
    lastActiveAt: new Date().toISOString(),
    capabilities: ['security', 'qa', 'audit', 'circuit-breaker'],
    metadata: { department: '品質監控部', model: 'nemotron-3.5-lightning:free' },
  },
  {
    id: 'wax',
    name: 'Wax',
    displayName: 'Wax (記憶整理員)',
    role: '記憶整理部 / Obsidian 知識庫與對話清洗',
    status: 'idle',
    lastActiveAt: new Date().toISOString(),
    capabilities: ['memory', 'obsidian', 'knowledge-base', 'cleanup'],
    metadata: { department: '記憶整理部', model: 'nemotron-3.5-super-vl:free' },
  },
]

export const useAgentStore = defineStore('agent', () => {
  // State
  const agents = ref<Agent[]>([...BUZZ_TEAM_AGENTS])
  const initialStatusMap: Record<string, AgentStatus | undefined> = {}
  BUZZ_TEAM_AGENTS.forEach(a => {
    initialStatusMap[a.id] = a.status
  })
  const agentStatusMap = ref<Record<string, AgentStatus | undefined>>(initialStatusMap)
  const isConnected = ref(false)
  const wsClient = ref<WebSocket | null>(null)
  const reconnectAttempts = ref(0)
  const maxReconnectAttempts = 5
  const reconnectDelay = 3000

  // API composable
  const api = useApi()
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  // Computed
  const onlineAgents = computed(() =>
    agents.value.filter(a => agentStatusMap.value[a.id] !== 'offline' && agentStatusMap.value[a.id] !== undefined)
  )
  const agentsByStatus = computed(() => {
    const grouped: Record<AgentStatus, Agent[]> = {
      idle: [],
      working: [],
      blocked: [],
      offline: [],
      error: [],
    }
    agents.value.forEach(agent => {
      const status = agentStatusMap.value[agent.id] ?? 'offline'
      grouped[status].push(agent)
    })
    return grouped
  })
  const agentCount = computed(() => agents.value.length)
  const onlineCount = computed(() => onlineAgents.value.length)

  // Actions
  function setAgents(list: Agent[]) {
    agents.value = list
  }

  async function initAgents() {
    isLoading.value = true
    error.value = null
    try {
      const remoteAgents = await api.fetchAgents()
      if (remoteAgents && remoteAgents.length > 0) {
        setAgents(remoteAgents)
        remoteAgents.forEach(a => {
          agentStatusMap.value[a.id] = a.status
        })
      }
    } catch {
      // 離線韌性降級：使用蜂群 6 大特工基礎資料
      if (agents.value.length === 0) {
        setAgents(BUZZ_TEAM_AGENTS)
      }
    } finally {
      isLoading.value = false
    }
  }

  function addAgent(agent: Agent) {
    if (!agents.value.find(a => a.id === agent.id)) {
      agents.value.push(agent)
    }
  }

  function updateAgent(id: string, updates: Partial<Agent>) {
    const idx = agents.value.findIndex(a => a.id === id)
    if (idx !== -1) {
      agents.value[idx] = { ...agents.value[idx], ...updates }
    }
  }

  function removeAgent(id: string) {
    agents.value = agents.value.filter(a => a.id !== id)
    delete agentStatusMap.value[id]
  }

  function setAgentStatus(agentId: string, status: AgentStatus) {
    agentStatusMap.value[agentId] = status
    const agent = agents.value.find(a => a.id === agentId)
    if (agent) {
      agent.status = status
    }
  }

  function handleWSMessage(message: { type: string; payload: unknown; timestamp?: string }) {
    switch (message.type) {
      case 'agent_status': {
        if (typeof message.payload === 'object' && message.payload !== null && 'agentId' in message.payload) {
          const { agentId, status } = message.payload as { agentId: string; status: AgentStatus }
          setAgentStatus(agentId, status)
        } else if (Array.isArray(message.payload)) {
          const list = message.payload as Agent[]
          setAgents(list)
          list.forEach(a => {
            agentStatusMap.value[a.id] = a.status
          })
        }
        break
      }
      case 'agent_snapshot':
      case 'agent_status_pulse': {
        const list = message.payload as Agent[]
        if (Array.isArray(list) && list.length > 0) {
          setAgents(list)
          list.forEach(a => {
            agentStatusMap.value[a.id] = a.status
          })
        }
        // 觸發遙測動態脈衝記錄
        try {
          useTelemetryStore().recordPulse()
        } catch {
          // 靜默降級
        }
        break
      }
      case 'task_progress': {
        console.log('Task progress:', message.payload)
        break
      }
    }
  }

  let pollingTimer: ReturnType<typeof setInterval> | null = null

  function startPolling(intervalMs = 5000) {
    if (pollingTimer) return
    pollingTimer = setInterval(() => {
      // 軌道 A：定時輪詢保護（當 WebSocket 未連線時無感平滑降級）
      if (!isConnected.value) {
        initAgents()
      }
    }, intervalMs)
  }

  function stopPolling() {
    if (pollingTimer) {
      clearInterval(pollingTimer)
      pollingTimer = null
    }
  }

  function autoConnect() {
    initAgents()
    const host = window.location.hostname || '127.0.0.1'
    const wsUrl = `ws://${host}:8080/ws`
    connectWS(wsUrl)
    startPolling(5000)
  }

  function connectWS(url: string) {
    if (wsClient.value?.readyState === WebSocket.OPEN) return

    wsClient.value = new WebSocket(url)

    wsClient.value.onopen = () => {
      isConnected.value = true
      reconnectAttempts.value = 0
      console.log('WebSocket connected')
    }

    wsClient.value.onmessage = event => {
      try {
        const message: WSMessage = JSON.parse(event.data)
        handleWSMessage(message)
      } catch (e) {
        console.error('Failed to parse WS message:', e)
      }
    }

    wsClient.value.onclose = () => {
      isConnected.value = false
      console.log('WebSocket disconnected')
      attemptReconnect(url)
    }

    wsClient.value.onerror = error => {
      console.error('WebSocket error:', error)
    }
  }

  function disconnectWS() {
    if (wsClient.value) {
      wsClient.value.close()
      wsClient.value = null
    }
    isConnected.value = false
  }

  function attemptReconnect(url: string) {
    if (reconnectAttempts.value >= maxReconnectAttempts) {
      console.error('Max reconnect attempts reached')
      return
    }
    reconnectAttempts.value++
    setTimeout(() => {
      console.log(`Reconnecting... (attempt ${reconnectAttempts.value})`)
      connectWS(url)
    }, reconnectDelay * reconnectAttempts.value)
  }

  function sendWSMessage(type: WSMessage['type'], payload: unknown) {
    if (wsClient.value?.readyState === WebSocket.OPEN) {
      wsClient.value.send(JSON.stringify({ type, payload, timestamp: new Date().toISOString() }))
    }
  }

  return {
    // State
    agents,
    agentStatusMap,
    isConnected,
    wsClient,
    isLoading,
    error,
    // Computed
    onlineAgents,
    agentsByStatus,
    agentCount,
    onlineCount,
    // Actions
    setAgents,
    addAgent,
    updateAgent,
    removeAgent,
    setAgentStatus,
    connectWS,
    disconnectWS,
    sendWSMessage,
    initAgents,
    startPolling,
    stopPolling,
    autoConnect,
  }
})
