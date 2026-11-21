import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Task, TaskStatus, TaskPriority } from '@/types'
import { useApi } from '@/composables'

// 預設蜂群 Sprint 工單列表 (離線韌性架構)
const BUZZ_SPRINT_TASKS: Task[] = [
  {
    id: 'DASH-001',
    projectId: 'proj-1',
    title: '儀表板核心架構與 API 串接',
    description: '建立 Vue 3 + Pinia + Vite 架構，串接 8080 微服務 /health 與 /api/triggers',
    status: 'done',
    priority: 'high',
    assigneeId: 'honey2',
    reporterId: 'user-1',
    sprintId: 'sprint-2',
    storyPoints: 8,
    tags: ['frontend', 'architecture', 'api', 'vite'],
    dependencies: [],
    createdAt: '2026-11-15T00:00:00Z',
    updatedAt: '2026-11-20T00:00:00Z',
  },
  {
    id: 'DASH-002',
    projectId: 'proj-1',
    title: '組織架構視圖與特工狀態環',
    description: 'Fizz、Honey、Pollen、Sentry 等六大特工拓撲節點，狀態環即時反映 idle/working/blocked',
    status: 'done',
    priority: 'high',
    assigneeId: 'honey2',
    reporterId: 'user-1',
    sprintId: 'sprint-2',
    storyPoints: 5,
    tags: ['frontend', 'visualization', 'd3', 'a11y'],
    dependencies: ['DASH-001'],
    createdAt: '2026-11-15T00:00:00Z',
    updatedAt: '2026-11-20T00:00:00Z',
  },
  {
    id: 'DASH-003',
    projectId: 'proj-1',
    title: '任務看板與即時進度',
    description: '看板欄位拖放、虛擬滾動、樂觀更新與進度條',
    status: 'done',
    priority: 'medium',
    assigneeId: 'honey2',
    reporterId: 'user-1',
    sprintId: 'sprint-2',
    storyPoints: 5,
    tags: ['frontend', 'kanban', 'dnd', 'realtime'],
    dependencies: ['DASH-001'],
    createdAt: '2026-11-15T00:00:00Z',
    updatedAt: '2026-11-20T00:00:00Z',
  },
  {
    id: 'GATE-001',
    projectId: 'proj-1',
    title: 'Triple Gate 防線 1 (Syntax Gate)',
    description: '離線語意檢查門禁與 buzz pack 整合',
    status: 'done',
    priority: 'critical',
    assigneeId: 'pollen',
    reporterId: 'user-1',
    sprintId: 'sprint-2',
    storyPoints: 8,
    tags: ['backend', 'security', 'rust'],
    dependencies: [],
    createdAt: '2026-11-18T00:00:00Z',
    updatedAt: '2026-11-19T00:00:00Z',
  },
  {
    id: 'GATE-002',
    projectId: 'proj-1',
    title: 'Triple Gate 防線 2 (Semantic Gate)',
    description: '契約檢查閘與狀態機驗證',
    status: 'done',
    priority: 'high',
    assigneeId: 'pollen',
    reporterId: 'user-1',
    sprintId: 'sprint-2',
    storyPoints: 5,
    tags: ['backend', 'contract', 'rust'],
    dependencies: ['GATE-001'],
    createdAt: '2026-11-19T00:00:00Z',
    updatedAt: '2026-11-19T00:00:00Z',
  },
  {
    id: 'GATE-003',
    projectId: 'proj-1',
    title: 'Triple Gate 防線 3 (Fingerprint Gate)',
    description: '構建指紋比對與 3 次連續失敗熔斷器（Circuit Breaker）',
    status: 'done',
    priority: 'critical',
    assigneeId: 'pollen',
    reporterId: 'user-1',
    sprintId: 'sprint-2',
    storyPoints: 8,
    tags: ['backend', 'fingerprint', 'circuit-breaker'],
    dependencies: ['GATE-002'],
    createdAt: '2026-11-19T00:00:00Z',
    updatedAt: '2026-11-20T00:00:00Z',
  },
  {
    id: 'SPRINT-3-PLAN',
    projectId: 'proj-1',
    title: '重大升級：投資部 3 位新成員組建 (Nectar, Comb, Royal)',
    description: '籌備投資部作戰室、註冊特工至 managed-agents.json 並配置專屬頻道',
    status: 'todo',
    priority: 'high',
    assigneeId: 'fizz',
    reporterId: 'user-1',
    sprintId: 'sprint-3',
    storyPoints: 13,
    tags: ['investment', 'multi-agent', 'planning'],
    dependencies: ['GATE-003', 'DASH-003'],
    createdAt: '2026-11-20T00:00:00Z',
    updatedAt: '2026-11-20T00:00:00Z',
  },
]

export const useTaskStore = defineStore('task', () => {
  // State
  const tasks = ref<Task[]>([...BUZZ_SPRINT_TASKS])
  const currentTask = ref<Task | null>(null)
  const filters = ref<{
    status?: TaskStatus
    priority?: TaskPriority
    assigneeId?: string
    projectId?: string
    sprintId?: string
    search?: string
  }>({})
  const sortBy = ref<'title' | 'status' | 'priority' | 'createdAt' | 'updatedAt'>('createdAt')
  const sortOrder = ref<'asc' | 'desc'>('desc')
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  // API composable
  const api = useApi()

  // Computed
  const filteredTasks = computed(() => {
    let result = tasks.value

    if (filters.value.status) {
      result = result.filter(t => t.status === filters.value.status)
    }
    if (filters.value.priority) {
      result = result.filter(t => t.priority === filters.value.priority)
    }
    if (filters.value.assigneeId) {
      result = result.filter(t => t.assigneeId === filters.value.assigneeId)
    }
    if (filters.value.projectId) {
      result = result.filter(t => t.projectId === filters.value.projectId)
    }
    if (filters.value.sprintId) {
      result = result.filter(t => t.sprintId === filters.value.sprintId)
    }
    if (filters.value.search) {
      const search = filters.value.search.toLowerCase()
      result = result.filter(
        t => t.title.toLowerCase().includes(search) || t.description?.toLowerCase().includes(search)
      )
    }

    result.sort((a, b) => {
      const aVal = a[sortBy.value]
      const bVal = b[sortBy.value]
      if (aVal === undefined || bVal === undefined) return 0
      const comparison = aVal < bVal ? -1 : aVal > bVal ? 1 : 0
      return sortOrder.value === 'asc' ? comparison : -comparison
    })

    return result
  })

  const tasksByStatus = computed(() => {
    const grouped: Record<TaskStatus, Task[]> = {
      backlog: [],
      todo: [],
      in_progress: [],
      review: [],
      done: [],
      cancelled: [],
    }
    filteredTasks.value.forEach(task => {
      grouped[task.status].push(task)
    })
    return grouped
  })

  const inProgressCount = computed(() => tasksByStatus.value.in_progress?.length || 0)
  const completedCount = computed(() => tasksByStatus.value.done?.length || 0)
  const blockedCount = computed(() => tasksByStatus.value.backlog?.length || 0)

  const taskStats = computed(() => ({
    total: tasks.value.length,
    byStatus: Object.fromEntries(
      Object.entries(tasksByStatus.value).map(([status, list]) => [status, list.length])
    ),
    byPriority: tasks.value.reduce(
      (acc, task) => {
        const priority = task.priority as TaskPriority
        acc[priority] = (acc[priority] || 0) + 1
        return acc
      },
      {} as Record<TaskPriority, number>
    ),
  }))

  // Actions
  async function fetchTasks(_projectId?: string, _sprintId?: string) {
    isLoading.value = true
    error.value = null
    try {
      const fetchedTasks = await api.fetchTasks()
      if (fetchedTasks && fetchedTasks.length > 0) {
        tasks.value = fetchedTasks
      } else if (tasks.value.length === 0) {
        tasks.value = BUZZ_SPRINT_TASKS
      }
    } catch {
      // 離線韌性降級：使用蜂群 Sprint 工單列表
      if (tasks.value.length === 0) {
        tasks.value = BUZZ_SPRINT_TASKS
      }
    } finally {
      isLoading.value = false
    }
  }

  function setFilters(newFilters: typeof filters.value) {
    filters.value = { ...filters.value, ...newFilters }
  }

  function clearFilters() {
    filters.value = {}
  }

  function setSort(field: typeof sortBy.value, order?: typeof sortOrder.value) {
    sortBy.value = field
    if (order) sortOrder.value = order
  }

  function addTask(task: Task) {
    tasks.value.unshift(task)
  }

  function updateTask(id: string, updates: Partial<Task>) {
    const idx = tasks.value.findIndex(t => t.id === id)
    if (idx !== -1) {
      tasks.value[idx] = { ...tasks.value[idx], ...updates, updatedAt: new Date().toISOString() }
      if (currentTask.value?.id === id) {
        currentTask.value = tasks.value[idx]
      }
    }
  }

  function removeTask(id: string) {
    tasks.value = tasks.value.filter(t => t.id !== id)
    if (currentTask.value?.id === id) {
      currentTask.value = null
    }
  }

  function setCurrentTask(task: Task | null) {
    currentTask.value = task
  }

  function moveTask(taskId: string, newStatus: TaskStatus) {
    updateTask(taskId, { status: newStatus })
  }

  function clearError() {
    error.value = null
  }

  return {
    tasks,
    currentTask,
    filters,
    sortBy,
    sortOrder,
    isLoading,
    error,
    filteredTasks,
    tasksByStatus,
    inProgressCount,
    completedCount,
    blockedCount,
    taskStats,
    fetchTasks,
    setFilters,
    clearFilters,
    setSort,
    addTask,
    updateTask,
    removeTask,
    setCurrentTask,
    moveTask,
    clearError,
  }
})
