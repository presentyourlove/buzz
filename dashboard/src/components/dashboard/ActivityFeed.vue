<script setup lang="ts">
import { computed } from 'vue'
import { formatRelativeTime } from '@/utils'
import { Clock, CheckCircle, AlertCircle, Info, Loader2, Plus } from 'lucide-vue-next'

interface ActivityItem {
  id: string
  type: 'task_created' | 'task_updated' | 'task_completed' | 'agent_status' | 'notification'
  title: string
  description?: string
  timestamp: string
  actor?: { name: string; avatar?: string }
  metadata?: Record<string, unknown>
}

interface Props {
  items: ActivityItem[]
  loading?: boolean
  emptyMessage?: string
  maxItems?: number
}

const props = withDefaults(defineProps<Props>(), {
  loading: false,
  emptyMessage: '暫無活動記錄',
  maxItems: 10,
})

const activityIcons = {
  task_created: { icon: Plus, color: 'text-blue-500', bg: 'bg-blue-100 dark:bg-blue-900/30' },
  task_updated: { icon: Loader2, color: 'text-yellow-500', bg: 'bg-yellow-100 dark:bg-yellow-900/30' },
  task_completed: { icon: CheckCircle, color: 'text-green-500', bg: 'bg-green-100 dark:bg-green-900/30' },
  agent_status: { icon: Info, color: 'text-purple-500', bg: 'bg-purple-100 dark:bg-purple-900/30' },
  notification: { icon: AlertCircle, color: 'text-red-500', bg: 'bg-red-100 dark:bg-red-900/30' },
}

const displayedItems = computed(() => props.items.slice(0, props.maxItems))
</script>

<template>
  <div class="space-y-4">
    <div v-if="loading" class="space-y-4">
      <div v-for="i in 3" :key="i" class="animate-pulse flex gap-3">
        <div class="w-10 h-10 rounded-full bg-gray-200 dark:bg-gray-700" />
        <div class="flex-1 space-y-2">
          <div class="h-4 bg-gray-200 dark:bg-gray-700 rounded w-3/4" />
          <div class="h-3 bg-gray-200 dark:bg-gray-700 rounded w-1/2" />
        </div>
      </div>
    </div>

    <div v-else-if="displayedItems.length === 0" class="text-center py-8">
      <Clock class="w-12 h-12 text-gray-300 dark:text-gray-600 mx-auto mb-3" />
      <p class="text-gray-500 dark:text-gray-400">{{ emptyMessage }}</p>
    </div>

    <ul v-else class="space-y-4" role="list" aria-label="活動記錄">
      <li v-for="item in displayedItems" :key="item.id" class="flex gap-3">
        <div
          :class="[
            'w-10 h-10 rounded-full flex items-center justify-center flex-shrink-0',
            activityIcons[item.type].bg,
          ]"
          aria-hidden="true"
        >
          <component :is="activityIcons[item.type].icon" :class="['w-5 h-5', activityIcons[item.type].color]" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="flex items-start gap-2">
            <p class="text-sm font-medium text-gray-900 dark:text-gray-100">{{ item.title }}</p>
            <time class="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap ml-auto" :dateTime="item.timestamp">
              {{ formatRelativeTime(item.timestamp) }}
            </time>
          </div>
          <p v-if="item.description" class="mt-1 text-sm text-gray-600 dark:text-gray-400">{{ item.description }}</p>
          <div v-if="item.actor" class="mt-2 flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
            <span v-if="item.actor.avatar" class="w-5 h-5 rounded-full bg-gray-200 dark:bg-gray-700" />
            <span>{{ item.actor.name }}</span>
          </div>
        </div>
      </li>
    </ul>
  </div>
</template>
