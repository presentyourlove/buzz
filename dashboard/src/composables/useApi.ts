import { ref } from 'vue'
import type { Agent, AgentStatus, Task, TaskStatus, Project, ApiResponse, PaginatedResponse } from '@/types'

const API_BASE = '/api/v1'

// 內部狀態
const agents = ref<Agent[]>([])
const tasks = ref<Task[]>([])
const projects = ref<Project[]>([])
const isLoading = ref(false)
const error = ref<string | null>(null)
const healthStatus = ref<'healthy' | 'degraded' | 'down' | 'unknown'>('unknown')

// 通用請求函數
async function request<T>(endpoint: string, options?: RequestInit): Promise<ApiResponse<T>> {
  isLoading.value = true
  error.value = null
  try {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
      ...options,
    })

    const data = await response.json()

    if (!response.ok) {
      throw new Error(data.error?.message || `HTTP ${response.status}`)
    }

    return data
  } catch (e) {
    const message = e instanceof Error ? e.message : '未知錯誤'
    error.value = message
    throw e
  } finally {
    isLoading.value = false
  }
}

// 健康檢查 - 直連 8080 端口繞過 Vite proxy
async function checkHealth(): Promise<boolean> {
  try {
    const response = await fetch('http://127.0.0.1:8080/health', { method: 'GET' })
    if (response.ok) {
      const data = await response.json()
      healthStatus.value = data.status || 'healthy'
      return true
    }
    healthStatus.value = 'down'
    return false
  } catch {
    healthStatus.value = 'down'
    return false
  }
}

// 特工相關 API
async function fetchAgents(): Promise<Agent[]> {
  const response = await request<Agent[]>('/agents')
  if (response.success && response.data) {
    agents.value = response.data
  }
  return agents.value
}

async function fetchAgentById(id: string): Promise<Agent | null> {
  const response = await request<Agent>(`/agents/${id}`)
  return response.success && response.data ? response.data : null
}

async function updateAgentStatus(id: string, status: AgentStatus): Promise<Agent | null> {
  const response = await request<Agent>(`/agents/${id}/status`, {
    method: 'PATCH',
    body: JSON.stringify({ status }),
  })
  if (response.success && response.data) {
    const idx = agents.value.findIndex(a => a.id === id)
    if (idx !== -1) agents.value[idx] = response.data
    return response.data
  }
  return null
}

// 任務相關 API
async function fetchTasks(params?: { status?: TaskStatus; agentId?: string; limit?: number }): Promise<Task[]> {
  const query = new URLSearchParams()
  if (params?.status) query.set('status', params.status)
  if (params?.agentId) query.set('agent_id', params.agentId)
  if (params?.limit) query.set('limit', String(params.limit))
  const qs = query.toString() ? `?${query.toString()}` : ''

  const response = await request<PaginatedResponse<Task>>(`/tasks${qs}`)
  if (response.success && response.data) {
    tasks.value = response.data.items
  }
  return tasks.value
}

async function fetchTaskById(id: string): Promise<Task | null> {
  const response = await request<Task>(`/tasks/${id}`)
  return response.success && response.data ? response.data : null
}

async function updateTaskStatus(id: string, status: TaskStatus, progress?: number, errorMsg?: string): Promise<Task | null> {
  const response = await request<Task>(`/tasks/${id}/status`, {
    method: 'PATCH',
    body: JSON.stringify({ status, progress, error: errorMsg }),
  })
  if (response.success && response.data) {
    const idx = tasks.value.findIndex(t => t.id === id)
    if (idx !== -1) tasks.value[idx] = response.data
    return response.data
  }
  return null
}

// 專案相關 API
async function fetchProjects(): Promise<Project[]> {
  const response = await request<Project[]>('/projects')
  if (response.success && response.data) {
    projects.value = response.data
  }
  return projects.value
}

// 觸發器相關 API（針對主動觸發器微服務）
export interface Trigger {
  id: string
  name: string
  type: 'schedule' | 'webhook' | 'event' | 'manual'
  status: 'active' | 'inactive' | 'error'
  config: Record<string, unknown>
  lastRunAt?: string
  nextRunAt?: string
  createdAt: string
  updatedAt: string
}

export interface TriggerExecution {
  id: string
  triggerId: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  startedAt: string
  completedAt?: string
  result?: Record<string, unknown>
  error?: string
}

async function fetchTriggers(): Promise<Trigger[]> {
  const response = await request<Trigger[]>('/triggers')
  return response.success && response.data ? response.data : []
}

async function fetchTriggerExecutions(triggerId: string, limit = 50): Promise<TriggerExecution[]> {
  const response = await request<TriggerExecution[]>(`/triggers/${triggerId}/executions?limit=${limit}`)
  return response.success && response.data ? response.data : []
}

async function executeTrigger(triggerId: string): Promise<TriggerExecution | null> {
  const response = await request<TriggerExecution>(`/triggers/${triggerId}/execute`, {
    method: 'POST',
  })
  return response.success && response.data ? response.data : null
}

// 導出 composable
export function useApi() {
  return {
    // 狀態
    agents,
    tasks,
    projects,
    isLoading,
    error,
    healthStatus,

    // 健康檢查
    checkHealth,

    // 特工
    fetchAgents,
    fetchAgentById,
    updateAgentStatus,

    // 任務
    fetchTasks,
    fetchTaskById,
    updateTaskStatus,

    // 專案
    fetchProjects,

    // 觸發器
    fetchTriggers,
    fetchTriggerExecutions,
    executeTrigger,
  }
}

// 類型導出
export type { Agent, AgentStatus, Task, TaskStatus, Project, ApiResponse, PaginatedResponse }
