import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ThemeMode } from '@/types'

export const useAppStore = defineStore('app', () => {
  // State
  const isInitialized = ref(false)
  const themeMode = ref<ThemeMode>('system')
  const sidebarCollapsed = ref(false)
  const currentProjectId = ref<string | null>(null)
  const onlineStatus = ref(true)

  // Computed
  const isDark = computed(() => {
    if (themeMode.value === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches
    }
    return themeMode.value === 'dark'
  })

  const effectiveTheme = computed(() => (isDark.value ? 'dark' : 'light'))

  // Actions
  function initialize() {
    if (isInitialized.value) return

    // 從 localStorage 讀取主題偏好
    const savedTheme = localStorage.getItem('themeMode') as ThemeMode | null
    if (savedTheme) {
      themeMode.value = savedTheme
    }

    // 從 localStorage 讀取側邊欄狀態
    const savedSidebar = localStorage.getItem('sidebarCollapsed')
    if (savedSidebar !== null) {
      sidebarCollapsed.value = savedSidebar === 'true'
    }

    // 監聽系統主題變化
    if (themeMode.value === 'system') {
      window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme)
    }

    applyTheme()
    isInitialized.value = true
  }

  function applyTheme() {
    const root = document.documentElement
    if (isDark.value) {
      root.classList.add('dark')
    } else {
      root.classList.remove('dark')
    }
  }

  function setThemeMode(mode: ThemeMode) {
    themeMode.value = mode
    localStorage.setItem('themeMode', mode)

    if (mode === 'system') {
      window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme)
    } else {
      window.matchMedia('(prefers-color-scheme: dark)').removeEventListener('change', applyTheme)
    }

    applyTheme()
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
    localStorage.setItem('sidebarCollapsed', sidebarCollapsed.value.toString())
  }

  function setSidebarCollapsed(collapsed: boolean) {
    sidebarCollapsed.value = collapsed
    localStorage.setItem('sidebarCollapsed', collapsed.toString())
  }

  function setCurrentProject(projectId: string | null) {
    currentProjectId.value = projectId
  }

  function setOnlineStatus(status: boolean) {
    onlineStatus.value = status
  }

  return {
    // State
    isInitialized,
    themeMode,
    sidebarCollapsed,
    currentProjectId,
    onlineStatus,
    // Computed
    isDark,
    effectiveTheme,
    // Actions
    initialize,
    setThemeMode,
    toggleSidebar,
    setSidebarCollapsed,
    setCurrentProject,
    setOnlineStatus,
  }
})
