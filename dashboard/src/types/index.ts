// ============================================
// 核心領域類型定義
// ============================================

/** 專案基礎資訊 */
export interface Project {
  id: string
  name: string
  slug: string
  description?: string
  ownerId: string
  createdAt: string
  updatedAt: string
  isActive: boolean
}

/** 特工 / 代理人狀態 */
export type AgentStatus = 'idle' | 'working' | 'blocked' | 'offline' | 'error'

export interface Agent {
  id: string
  name: string
  displayName: string
  avatar?: string
  role: string
  status: AgentStatus
  currentTask?: string
  lastActiveAt: string
  capabilities: string[]
  metadata: Record<string, unknown>
}

/** 任務 / 工單 */
export type TaskStatus = 'backlog' | 'todo' | 'in_progress' | 'review' | 'done' | 'cancelled'
export type TaskPriority = 'low' | 'medium' | 'high' | 'critical'

export interface Task {
  id: string
  projectId: string
  title: string
  description?: string
  status: TaskStatus
  priority: TaskPriority
  assigneeId?: string
  reporterId: string
  sprintId?: string
  storyPoints?: number
  tags: string[]
  dependencies: string[] // 任務 ID 陣列
  createdAt: string
  updatedAt: string
  completedAt?: string
  dueDate?: string
}

/** Sprint 迭代 */
export interface Sprint {
  id: string
  projectId: string
  name: string
  goal?: string
  startDate: string
  endDate: string
  status: 'planning' | 'active' | 'completed' | 'cancelled'
  tasks: string[] // 任務 ID 陣列
}

/** 組織架構節點 */
export interface OrgNode {
  id: string
  name: string
  role: string
  avatar?: string
  children: OrgNode[]
  parentId?: string
  agentIds: string[] // 隸屬特工
  metadata: {
    department?: string
    level: number
    isExpanded: boolean
  }
}

/** 即時進度事件 */
export interface ProgressEvent {
  taskId: string
  agentId: string
  progress: number // 0-100
  status: TaskStatus
  message?: string
  timestamp: string
}

/** WebSocket 訊息類型 */
export type WSMessageType =
  | 'agent_status'
  | 'task_progress'
  | 'task_created'
  | 'task_updated'
  | 'task_deleted'
  | 'sprint_updated'
  | 'notification'

export interface WSMessage<T = unknown> {
  type: WSMessageType
  payload: T
  timestamp: string
  correlationId?: string
}

// ============================================
// API 回應包裝
// ============================================

export interface ApiResponse<T> {
  success: boolean
  data?: T
  error?: {
    code: string
    message: string
    details?: Record<string, unknown>
  }
  meta?: {
    page?: number
    pageSize?: number
    total?: number
  }
}

export interface PaginatedResponse<T> {
  items: T[]
  total: number
  page: number
  pageSize: number
  totalPages: number
}

// ============================================
// UI 狀態類型
// ============================================

export type ThemeMode = 'light' | 'dark' | 'system'

export type ProjectStatus = 'active' | 'archived' | 'on_hold'

export interface Toast {
  id: string
  type: 'success' | 'error' | 'warning' | 'info'
  title: string
  message?: string
  duration?: number
  action?: {
    label: string
    onClick: () => void
  }
  _dismissed?: boolean
}

export interface ModalState {
  isOpen: boolean
  title?: string
  content?: string
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full'
  onConfirm?: () => void | Promise<void>
  onCancel?: () => void
}
