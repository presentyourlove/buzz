<script setup lang="ts">
import { ref, computed } from 'vue'
import { useAppStore, useAuthStore } from '@/stores'
import { useAgentStore } from '@/stores/agent'
import { Sun, Moon, Monitor, Bell, ChevronDown, LogOut, User } from 'lucide-vue-next'
import Modal from '../common/Modal.vue'
import Button from '../common/Button.vue'
import { useToast } from '@/composables'

const appStore = useAppStore()
const authStore = useAuthStore()
const agentStore = useAgentStore()
const { success: _success, error: _error } = useToast()

const theme = computed(() => appStore.effectiveTheme)
const userMenuOpen = ref(false)
const logoutConfirmOpen = ref(false)

const toggleTheme = () => {
  const modes: ('light' | 'dark' | 'system')[] = ['light', 'dark', 'system']
  const currentIndex = modes.indexOf(appStore.themeMode)
  const nextIndex = (currentIndex + 1) % modes.length
  appStore.setThemeMode(modes[nextIndex])
  _success('主題已切換', `當前模式：${modes[nextIndex] === 'system' ? '跟隨系統' : modes[nextIndex]}`)
}

const handleLogout = () => {
  authStore.logout()
  userMenuOpen.value = false
  logoutConfirmOpen.value = false
  _success('已登出', '歡迎下次再來')
}
</script>

<template>
  <header
    :class="[
      'fixed top-0 right-0 z-30 h-16 bg-white/80 dark:bg-gray-800/80 backdrop-blur-sm border-b border-gray-200 dark:border-gray-700 flex items-center justify-between px-4 transition-all duration-300',
      { 'left-64': !appStore.sidebarCollapsed, 'left-16': appStore.sidebarCollapsed },
    ]"
    role="banner"
  >
    <div class="flex items-center gap-2">
      <span
        :class="[
          'inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium',
          agentStore.isConnected
            ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300'
            : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300'
        ]"
      >
        <span
          :class="[
            'w-2 h-2 rounded-full',
            agentStore.isConnected ? 'bg-green-500 animate-pulse' : 'bg-yellow-500'
          ]"
        />
        {{ agentStore.isConnected ? '即時連線 (WS 毫秒級)' : '定時輪詢 (5s 降級)' }}
      </span>
    </div>

    <div class="flex-1" />

    <div class="flex items-center gap-2">
      <!-- Theme Toggle -->
      <button
        @click="toggleTheme"
        class="p-2 rounded-lg text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
        :aria-label="`切換主題（當前：${theme === 'dark' ? '深色' : '淺色'}模式）`"
      >
        <Sun v-if="theme === 'dark'" class="w-5 h-5" />
        <Moon v-else-if="theme === 'light'" class="w-5 h-5" />
        <Monitor v-else class="w-5 h-5" />
      </button>

      <!-- Notifications -->
      <button class="relative p-2 rounded-lg text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" aria-label="通知">
        <Bell class="w-5 h-5" />
        <span class="absolute top-1 right-1 w-2 h-2 bg-red-500 rounded-full" />
      </button>

      <!-- User Menu -->
      <div class="relative">
        <button
          @click="userMenuOpen = !userMenuOpen"
          class="flex items-center gap-2 p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          aria-expanded="false"
          aria-haspopup="true"
        >
          <div class="w-8 h-8 rounded-full bg-primary-100 dark:bg-primary-900 flex items-center justify-center">
            <User class="w-5 h-5 text-primary-600 dark:text-primary-400" />
          </div>
          <span v-if="!appStore.sidebarCollapsed" class="text-sm font-medium text-gray-700 dark:text-gray-300">
            {{ authStore.userDisplayName }}
          </span>
          <ChevronDown class="w-4 h-4 text-gray-500" v-if="!appStore.sidebarCollapsed" />
        </button>

        <Transition name="dropdown">
          <div
            v-if="userMenuOpen"
            class="absolute right-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 py-1 z-50"
            role="menu"
          >
            <div class="px-4 py-2 border-b border-gray-200 dark:border-gray-700">
              <p class="text-sm font-medium text-gray-900 dark:text-gray-100">{{ authStore.userDisplayName }}</p>
              <p class="text-xs text-gray-500 dark:text-gray-400 truncate">{{ authStore.userRole }}</p>
            </div>
            <button
              class="flex items-center gap-2 w-full px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
              role="menuitem"
              @click="userMenuOpen = false"
            >
              <User class="w-4 h-4" />
              個人資料
            </button>
            <button
              class="flex items-center gap-2 w-full px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
              role="menuitem"
              @click="userMenuOpen = false"
            >
              <Sun class="w-4 h-4" />
              設定
            </button>
            <hr class="my-1 border-gray-200 dark:border-gray-700" />
            <button
              class="flex items-center gap-2 w-full px-4 py-2 text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
              role="menuitem"
              @click="logoutConfirmOpen = true; userMenuOpen = false"
            >
              <LogOut class="w-4 h-4" />
              登出
            </button>
          </div>
        </Transition>
      </div>
    </div>

    <!-- Logout Confirmation Modal -->
    <Modal
      v-model="logoutConfirmOpen"
      title="確認登出"
      size="sm"
      @close="logoutConfirmOpen = false"
    >
      <p class="text-gray-600 dark:text-gray-400">確定要登出嗎？</p>
      <template #actions>
        <Button variant="ghost" @click="logoutConfirmOpen = false">取消</Button>
        <Button variant="danger" @click="handleLogout">登出</Button>
      </template>
    </Modal>
  </header>
</template>

<style scoped>
.dropdown-enter-active,
.dropdown-leave-active {
  transition: all 0.15s ease;
}
.dropdown-enter-from {
  opacity: 0;
  transform: translateY(-8px);
}
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
