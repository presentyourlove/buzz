<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useTaskStore } from '@/stores/task'
import { useAppStore } from '@/stores/app'
import { PageHeader } from '@/components'
import TaskProgressBar from '@/components/dashboard/TaskProgressBar.vue'

const taskStore = useTaskStore()
const appStore = useAppStore()

// Column configuration
const columns = [
  { key: 'backlog', label: '待辦清單', color: 'gray' },
  { key: 'todo', label: '待處理', color: 'blue' },
  { key: 'in_progress', label: '進行中', color: 'yellow' },
  { key: 'review', label: '審核中', color: 'purple' },
  { key: 'done', label: '已完成', color: 'green' },
  { key: 'cancelled', label: '已取消', color: 'red' },
] as const

type ColumnKey = typeof columns[number]['key']

// Initialize tasks from API on mount
onMounted(() => {
  taskStore.fetchTasks()
  appStore.setSidebarCollapsed(false)
})

// Computed: get tasks grouped by status
const tasksByStatus = computed(() => taskStore.tasksByStatus)

// Calculate progress for each column (based on task count vs total)
const columnProgress = computed(() => {
  const total = taskStore.tasks.length || 1
  const progress: Record<ColumnKey, number> = {
    backlog: 0,
    todo: 0,
    in_progress: 0,
    review: 0,
    done: 0,
    cancelled: 0,
  }
  columns.forEach(col => {
    const count = tasksByStatus.value[col.key]?.length || 0
    progress[col.key] = Math.round((count / total) * 100)
  })
  return progress
})

function formatDate(dateStr: string): string {
  const date = new Date(dateStr)
  return date.toLocaleDateString('zh-TW', { month: 'short', day: 'numeric' })
}

function getPriorityColor(priority: string): string {
  const colors: Record<string, string> = {
    critical: 'text-red-600 bg-red-50 dark:bg-red-900/20',
    high: 'text-orange-600 bg-orange-50 dark:bg-orange-900/20',
    medium: 'text-yellow-600 bg-yellow-50 dark:bg-yellow-900/20',
    low: 'text-green-600 bg-green-50 dark:bg-green-900/20',
  }
  return colors[priority] || 'text-gray-600 bg-gray-50 dark:bg-gray-900/20'
}

function getPriorityIcon(priority: string): string {
  const icons: Record<string, string> = {
    critical: '🔴',
    high: '🟠',
    medium: '🟡',
    low: '🟢',
  }
  return icons[priority] || '⚪'
}
</script>

<template>
  <div class="p-6">
    <PageHeader
      title="任務看板"
      subtitle="拖拽式任務管理，即時追蹤 Sprint 進度與阻塞項"
      :breadcrumbs="[
        { label: '首頁', href: '/' },
        { label: '任務看板' },
      ]"
    />

    <!-- Loading State -->
    <div v-if="taskStore.isLoading && taskStore.tasks.length === 0" class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-10 w-10 border-3 border-blue-500 border-t-transparent" />
    </div>

    <!-- Error State (僅在完全無任務資料時顯示) -->
    <div v-else-if="taskStore.error && taskStore.tasks.length === 0" class="card p-6 bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800">
      <div class="flex items-center gap-3 text-red-700 dark:text-red-300">
        <span class="text-2xl">⚠️</span>
        <div>
          <p class="font-medium">載入任務資料失敗</p>
          <p class="text-sm">{{ taskStore.error }}</p>
          <button
            @click="taskStore.fetchTasks()"
            class="mt-2 text-sm underline hover:no-underline"
          >
            重試
          </button>
        </div>
      </div>
    </div>

    <!-- Kanban Board -->
    <div v-else class="space-y-6">
      <!-- Summary Stats -->
      <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
        <div class="card p-4 border-l-4 border-gray-400">
          <p class="text-sm text-gray-500 dark:text-gray-400">總任務數</p>
          <p class="text-2xl font-bold text-gray-900 dark:text-gray-100">{{ taskStore.taskStats.total }}</p>
        </div>
        <div class="card p-4 border-l-4 border-yellow-500">
          <p class="text-sm text-gray-500 dark:text-gray-400">進行中</p>
          <p class="text-2xl font-bold text-yellow-700 dark:text-yellow-300">{{ taskStore.inProgressCount }}</p>
        </div>
        <div class="card p-4 border-l-4 border-green-500">
          <p class="text-sm text-gray-500 dark:text-gray-400">已完成</p>
          <p class="text-2xl font-bold text-green-700 dark:text-green-300">{{ taskStore.completedCount }}</p>
        </div>
        <div class="card p-4 border-l-4 border-blue-500">
          <p class="text-sm text-gray-500 dark:text-gray-400">完成率</p>
          <p class="text-2xl font-bold text-blue-700 dark:text-blue-300">
            {{ taskStore.taskStats.total > 0 ? Math.round((taskStore.completedCount / taskStore.taskStats.total) * 100) : 0 }}%
          </p>
        </div>
      </div>

      <!-- Kanban Columns -->
      <div class="flex gap-4 overflow-x-auto pb-4" style="min-width: 100%;">
        <template v-for="col in columns" :key="col.key">
          <div
            class="flex-shrink-0 w-72 min-w-72 max-h-[calc(100vh-300px)] overflow-y-auto"
            :style="`scrollbar-thin scrollbar-thumb-gray-300 dark:scrollbar-thumb-gray-600`"
          >
            <!-- Column Header -->
            <div class="sticky top-0 z-10 mb-3 p-3 rounded-t-lg" :class="`bg-${col.color}-100 dark:bg-${col.color}-900/30`">
              <div class="flex items-center justify-between">
                <h3 class="font-semibold text-gray-900 dark:text-gray-100 capitalize">
                  {{ col.label }}
                </h3>
                <div class="flex items-center gap-2">
                  <TaskProgressBar
                    :progress="columnProgress[col.key]"
                    :status="col.key"
                    size="sm"
                    :show-label="true"
                    striped
                    :animated="col.key === 'in_progress'"
                  />
                  <span class="text-sm font-mono text-gray-600 dark:text-gray-400">
                    {{ tasksByStatus[col.key]?.length || 0 }}
                  </span>
                </div>
              </div>
            </div>

            <!-- Task Cards -->
            <div class="space-y-3 p-1 min-h-[200px] bg-gray-50/50 dark:bg-gray-800/50 rounded-b-lg">
              <template v-if="tasksByStatus[col.key] && tasksByStatus[col.key].length > 0">
                <div
                  v-for="task in tasksByStatus[col.key]"
                  :key="task.id"
                  class="card p-3 hover:shadow-md transition-shadow cursor-pointer border-l-4"
                  :class="`border-${getPriorityColor(task.priority).split(' ')[1].replace('bg-', '')}`"
                >
                  <div class="flex items-start gap-2">
                    <span class="text-lg mt-0.5">{{ getPriorityIcon(task.priority) }}</span>
                    <div class="flex-1 min-w-0">
                      <h4 class="font-medium text-gray-900 dark:text-gray-100 truncate">{{ task.title }}</h4>
                      <p class="text-xs text-gray-500 dark:text-gray-400 line-clamp-2 mt-1">{{ task.description }}</p>
                      <div class="flex items-center gap-2 mt-2 text-xs">
                        <span
                          :class="['px-2 py-0.5 rounded text-xs font-medium', getPriorityColor(task.priority)]"
                        >
                          {{ task.priority }}
                        </span>
                        <span class="text-gray-400 dark:text-gray-500 font-mono">{{ task.id }}</span>
                      </div>
                      <div class="flex items-center gap-2 mt-1 text-xs text-gray-400 dark:text-gray-500">
                        <span class="flex items-center gap-1">
                          📅 {{ formatDate(task.createdAt) }}
                        </span>
                        <span v-if="task.storyPoints" class="flex items-center gap-1">
                          ⭐ {{ task.storyPoints }} pts
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              </template>
              <div v-else class="text-center py-8 text-gray-400 dark:text-gray-500 text-sm">
                暫無任務
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
