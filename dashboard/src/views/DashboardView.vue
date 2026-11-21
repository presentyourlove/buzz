<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useAppStore } from '@/stores/app'
import { useAgentStore } from '@/stores/agent'
import { useTaskStore } from '@/stores/task'
import { useTelemetryStore } from '@/stores/telemetry'
import {
  PageHeader,
  StatsCard,
  AgentStatusRing,
  TaskProgressBar,
  ActivityFeed,
  LiveTelemetryChart,
} from '@/components'
import { Users, CheckCircle, TrendingUp, Activity, Zap } from 'lucide-vue-next'

const appStore = useAppStore()
const agentStore = useAgentStore()
const taskStore = useTaskStore()
const telemetryStore = useTelemetryStore()

// Stats cards with string icons and proper trend types
const statsCards = computed(() => [
  {
    title: '線上特工',
    value: agentStore.onlineCount,
    icon: 'Users',
    trend: 'up' as const,
    trendUp: true,
    color: 'blue',
  },
  {
    title: '進行中任務',
    value: taskStore.inProgressCount,
    icon: 'Zap',
    trend: 'up' as const,
    trendUp: false,
    color: 'yellow',
  },
  {
    title: '已完成任務',
    value: taskStore.completedCount,
    icon: 'CheckCircle',
    trend: 'up' as const,
    trendUp: true,
    color: 'green',
  },
  {
    title: '阻塞任務',
    value: taskStore.blockedCount,
    icon: 'AlertTriangle',
    trend: 'down' as const,
    trendUp: true,
    color: 'red',
  },
])

const agentStatusDistribution = computed((): Record<'idle' | 'working' | 'blocked' | 'offline' | 'error', number> => {
  const agents = agentStore.agents
  const distribution: Record<'idle' | 'working' | 'blocked' | 'offline' | 'error', number> = {
    idle: 0,
    working: 0,
    blocked: 0,
    offline: 0,
    error: 0,
  }
  agents.forEach(agent => {
    const status = agentStore.agentStatusMap[agent.id] || 'offline'
    distribution[status]++
  })
  return distribution
})

const recentTasks = computed(() => {
  return taskStore.tasks
    .filter(t => t.status === 'in_progress' || t.status === 'review')
    .slice(0, 5)
})

const displayTasks = computed(() => {
  if (recentTasks.value.length > 0) return recentTasks.value
  return taskStore.tasks.slice(0, 5)
})

// 即時動態事件流 (對接 TelemetryStore 即時脈衝與任務事件，隨秒動態刷新)
const recentActivity = computed(() => {
  const activities: Array<{
    id: string
    type: 'task_created' | 'task_updated' | 'task_completed' | 'agent_status' | 'notification'
    title: string
    description?: string
    timestamp: string
    actor?: { name: string; avatar?: string }
  }> = []

  // 1. 優先推入即時遙測事件 (即時心跳脈衝、探針紀錄)
  telemetryStore.liveEvents.slice(0, 8).forEach(evt => {
    activities.push({
      id: evt.id,
      type: evt.type === 'heartbeat' ? 'agent_status' : evt.type === 'probe' ? 'notification' : 'agent_status',
      title: evt.title,
      description: evt.description,
      timestamp: evt.timestamp.toISOString(),
      actor: { name: 'Buzz 守護蜂' },
    })
  })

  // 2. 補充工單任務事件
  taskStore.tasks.slice(0, 2).forEach(task => {
    activities.push({
      id: `activity-${task.id}`,
      type: 'task_updated',
      title: `任務工單：${task.title}`,
      description: `狀態：${task.status} | 優先級：${task.priority}`,
      timestamp: new Date().toISOString(),
      actor: { name: '調度總管' },
    })
  })

  return activities.slice(0, 10)
})

const breadcrumbs = computed(() => [
  { label: '儀表板', href: '/dashboard' },
])

onMounted(() => {
  telemetryStore.startTelemetryTicker()
})
</script>

<template>
  <div class="p-6 space-y-6">
    <PageHeader
      title="儀表板"
      subtitle="蜂群專案總覽與即時動能監控"
      :breadcrumbs="breadcrumbs"
    >
      <template #actions>
        <div class="flex items-center gap-2">
          <button
            class="btn btn-secondary flex items-center gap-1.5 text-xs py-1.5"
            @click="telemetryStore.triggerPingProbe()"
          >
            <Zap class="w-3.5 h-3.5 text-amber-500" />
            發送探針
          </button>
          <button class="btn btn-primary text-xs py-1.5" @click="appStore.toggleSidebar">
            切換側邊欄
          </button>
        </div>
      </template>
    </PageHeader>

    <!-- 即時動能與心跳波形監控組件 (Live Telemetry Chart) -->
    <LiveTelemetryChart />

    <!-- Stats Cards Row -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
      <StatsCard
        v-for="card in statsCards"
        :key="card.title"
        :title="card.title"
        :value="card.value"
        :icon="card.icon"
        :trend="card.trend"
        :trend-up="card.trendUp"
        :color="card.color"
      />
    </div>

    <!-- Main Content Grid -->
    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
      <!-- Left Column: Agent Status & Task Progress -->
      <div class="lg:col-span-2 space-y-6">
        <!-- Agent Status Rings -->
        <section class="card p-6">
          <div class="flex items-center justify-between mb-4">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
              <Users class="w-5 h-5 text-primary-500" />
              特工狀態總覽
            </h2>
            <span class="inline-flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400 font-medium">
              <span class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
              生命徵象活躍
            </span>
          </div>
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            <AgentStatusRing
              v-for="(_count, status) in agentStatusDistribution"
              :key="status"
              :status="status"
              :show-label="true"
            />
          </div>
        </section>

        <!-- Task Progress List -->
        <section class="card p-6">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4 flex items-center gap-2">
            <CheckCircle class="w-5 h-5 text-primary-500" />
            任務進度追蹤
          </h2>
          <div class="space-y-3">
            <TaskProgressBar
              v-for="task in displayTasks"
              :key="task.id"
              :progress="task.status === 'done' ? 100 : task.status === 'in_progress' ? 50 : task.status === 'review' ? 75 : 10"
              :status="task.status"
              :animated="task.status === 'in_progress'"
              :show-label="true"
            />
            <div v-if="displayTasks.length === 0" class="text-center py-8 text-gray-500 dark:text-gray-400">
              暫無任務資料
            </div>
          </div>
        </section>
      </div>

      <!-- Right Column: Activity Feed -->
      <div class="lg:col-span-1">
        <section class="card p-6 h-full flex flex-col justify-between">
          <div>
            <div class="flex items-center justify-between mb-4">
              <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
                <Activity class="w-5 h-5 text-primary-500" />
                即時動態事件流
              </h2>
              <span class="text-xs text-gray-400 font-mono">
                每 3 秒自動同步
              </span>
            </div>
            <ActivityFeed
              :items="recentActivity"
              :max-items="8"
              empty-message="暫無活動記錄"
            />
          </div>
          <div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
            <a href="#" class="text-sm text-primary-600 dark:text-primary-400 hover:underline flex items-center justify-center gap-1">
              <TrendingUp class="w-4 h-4" />
              查看完整活動日誌
            </a>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>
