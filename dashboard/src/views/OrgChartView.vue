<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useAgentStore } from '@/stores/agent'
import { useAppStore } from '@/stores/app'
import { PageHeader } from '@/components'
import AgentStatusRing from '@/components/dashboard/AgentStatusRing.vue'

const agentStore = useAgentStore()
const appStore = useAppStore()

// Initialize agents from API on mount
onMounted(() => {
  agentStore.initAgents()
  appStore.setSidebarCollapsed(false)
})

// Computed: group agents by status for display
const agentsByStatus = computed(() => agentStore.agentsByStatus)
const statusOrder = ['working', 'idle', 'blocked', 'offline', 'error'] as const

function getStatusIcon(status: string) {
  const icons: Record<string, string> = {
    working: '🔄',
    idle: '💤',
    blocked: '⚠️',
    offline: '⭕',
    error: '❌',
  }
  return icons[status] || '❓'
}

function getStatusColor(status: string): string {
  const colors: Record<string, string> = {
    working: 'text-blue-500',
    idle: 'text-green-500',
    blocked: 'text-yellow-500',
    offline: 'text-gray-400',
    error: 'text-red-500',
  }
  return colors[status] || 'text-gray-500'
}
</script>

<template>
  <div class="p-6">
    <PageHeader
      title="組織架構圖"
      subtitle="視覺化呈現專案成員、特工編制與協作關係"
      :breadcrumbs="[
        { label: '首頁', href: '/' },
        { label: '組織架構圖' },
      ]"
    />

    <!-- Loading State -->
    <div v-if="agentStore.isLoading && agentStore.agentCount === 0" class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-10 w-10 border-3 border-blue-500 border-t-transparent" />
    </div>

    <!-- Error State (僅在無任何可用特工資料時顯示) -->
    <div v-else-if="agentStore.error && agentStore.agentCount === 0" class="card p-6 bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800">
      <div class="flex items-center gap-3 text-red-700 dark:text-red-300">
        <span class="text-2xl">⚠️</span>
        <div>
          <p class="font-medium">載入特工資料失敗</p>
          <p class="text-sm">{{ agentStore.error }}</p>
          <button
            @click="agentStore.initAgents()"
            class="mt-2 text-sm underline hover:no-underline"
          >
            重試
          </button>
        </div>
      </div>
    </div>

    <!-- Agent Grid -->
    <div v-else class="space-y-6">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold">特工狀態總覽 ({{ agentStore.agentCount }} 位)</h2>
        <div class="flex items-center gap-4 text-sm text-gray-500 dark:text-gray-400">
          <span class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-green-500" /> 線上: {{ agentStore.onlineCount }}
          </span>
          <span class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-blue-500 animate-pulse" /> 忙碌: {{ agentStore.agentsByStatus.working?.length || 0 }}
          </span>
          <span class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-yellow-500" /> 阻塞: {{ agentStore.agentsByStatus.blocked?.length || 0 }}
          </span>
        </div>
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        <!-- Render agents grouped by status -->
        <template v-for="status in statusOrder" :key="status">
          <template v-if="agentsByStatus[status] && agentsByStatus[status].length > 0">
            <div class="col-span-full">
              <h3 class="text-xs font-medium uppercase tracking-wider text-gray-400 dark:text-gray-500 mb-2 flex items-center gap-1">
                <span :class="getStatusColor(status)">{{ getStatusIcon(status) }}</span>
                {{ status.charAt(0).toUpperCase() + status.slice(1) }} ({{ agentsByStatus[status].length }})
              </h3>
            </div>
            <div
              v-for="agent in agentsByStatus[status]"
              :key="agent.id"
              class="card p-4 hover:shadow-md transition-shadow group"
            >
              <div class="flex items-center gap-3">
                <AgentStatusRing
                  :status="agent.status as 'idle' | 'working' | 'blocked' | 'offline' | 'error'"
                  :size="48"
                  :show-label="true"
                />
                <div class="flex-1 min-w-0">
                  <h4 class="font-medium text-gray-900 dark:text-gray-100 truncate">{{ agent.name }}</h4>
                  <p class="text-xs text-gray-500 dark:text-gray-400 truncate">{{ agent.role }}</p>
                  <p class="text-xs text-gray-400 dark:text-gray-500 font-mono">{{ agent.id }}</p>
                </div>
                <div class="text-right">
                  <span
                    :class="[
                      'inline-flex items-center px-2 py-1 rounded-full text-xs font-medium',
                      agent.status === 'working' ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300' :
                      agent.status === 'idle' ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300' :
                      agent.status === 'blocked' ? 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300' :
                      agent.status === 'error' ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300' :
                      'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300'
                    ]"
                  >
                    {{ agent.status }}
                  </span>
                </div>
              </div>
            </div>
          </template>
        </template>
      </div>

      <!-- Empty State -->
      <div v-if="agentStore.agentCount === 0" class="card p-12 text-center">
        <p class="text-gray-500 dark:text-gray-400 text-lg">暫無特工資料</p>
        <p class="text-sm text-gray-400 dark:text-gray-500 mt-2">請確認微服務 (port 8080) 正在運行</p>
        <button
          @click="agentStore.initAgents()"
          class="mt-4 px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors"
        >
          重新載入
        </button>
      </div>
    </div>
  </div>
</template>
