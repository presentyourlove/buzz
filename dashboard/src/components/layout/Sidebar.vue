<script setup lang="ts">
import { computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAppStore } from '@/stores'
import {
  LayoutDashboard,
  Users,
  Kanban,
  Settings,
  ChevronLeft,
  ChevronRight,
} from 'lucide-vue-next'

const router = useRouter()
const route = useRoute()
const appStore = useAppStore()

const navItems = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard },
  { name: '組織圖', href: '/org-chart', icon: Users },
  { name: '任務看板', href: '/kanban', icon: Kanban },
  { name: '設定', href: '/settings', icon: Settings },
] as const

const sidebarWidth = computed(() => (appStore.sidebarCollapsed ? 'w-16' : 'w-64'))
const labelVisible = computed(() => !appStore.sidebarCollapsed)

const toggleSidebar = () => {
  appStore.toggleSidebar()
}

const navigate = (path: string) => {
  router.push(path)
}
</script>

<template>
  <aside
    :class="[
      'fixed inset-y-0 left-0 z-40 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 transition-all duration-300 flex flex-col',
      sidebarWidth,
    ]"
    aria-label="側邊導航"
  >
    <!-- Logo / Brand -->
    <div class="flex items-center justify-between h-16 px-4 border-b border-gray-200 dark:border-gray-700">
      <div v-if="labelVisible" class="flex items-center gap-2">
        <svg class="w-8 h-8 text-primary-600" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
        </svg>
        <span class="text-lg font-bold text-gray-900 dark:text-gray-100">Buzz 監控</span>
      </div>
      <button
        @click="toggleSidebar"
        class="p-1.5 rounded-lg text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
        :aria-label="appStore.sidebarCollapsed ? '展開側邊欄' : '收合側邊欄'"
      >
        <ChevronLeft v-if="!appStore.sidebarCollapsed" class="w-5 h-5" />
        <ChevronRight v-else class="w-5 h-5" />
      </button>
    </div>

    <!-- Navigation -->
    <nav class="flex-1 px-3 py-4 space-y-1 overflow-y-auto" role="navigation" aria-label="主導航">
      <ul class="space-y-1" role="list">
        <li v-for="item in navItems" :key="item.name">
          <button
            @click="navigate(item.href)"
            :class="[
              'flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors',
              'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
              route.path === item.href
                ? 'bg-primary-50 text-primary-700 dark:bg-primary-900/20 dark:text-primary-300'
                : 'text-gray-700 hover:bg-gray-100 hover:text-gray-900 dark:text-gray-300 dark:hover:bg-gray-700 dark:hover:text-gray-100',
            ]"
            :aria-current="route.path === item.href ? 'page' : undefined"
          >
            <component :is="item.icon" class="w-5 h-5 flex-shrink-0" aria-hidden="true" />
            <span v-if="labelVisible" class="truncate">{{ item.name }}</span>
          </button>
        </li>
      </ul>
    </nav>

    <!-- Footer -->
    <div class="p-4 border-t border-gray-200 dark:border-gray-700">
      <div v-if="labelVisible" class="text-xs text-gray-500 dark:text-gray-400 text-center">
        Buzz 監控面板 v0.0.1
      </div>
    </div>
  </aside>
</template>
