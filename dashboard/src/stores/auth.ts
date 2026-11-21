import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Agent } from '@/types'

export const useAuthStore = defineStore('auth', () => {
  // State
  const currentUser = ref<Agent | null>(null)
  const isAuthenticated = ref(false)
  const token = ref<string | null>(null)
  const refreshToken = ref<string | null>(null)
  const permissions = ref<string[]>([])

  // Computed
  const userDisplayName = computed(() => currentUser.value?.displayName ?? currentUser.value?.name ?? '訪客')
  const userAvatar = computed(() => currentUser.value?.avatar)
  const userRole = computed(() => currentUser.value?.role)
  const hasPermission = computed(() => (perm: string) => permissions.value.includes(perm))

  // Actions
  function login(user: Agent, accessToken: string, refresh?: string) {
    currentUser.value = user
    token.value = accessToken
    if (refresh) refreshToken.value = refresh
    isAuthenticated.value = true
    persistAuth()
  }

  function logout() {
    currentUser.value = null
    token.value = null
    refreshToken.value = null
    permissions.value = []
    isAuthenticated.value = false
    clearAuth()
  }

  function updateUser(updates: Partial<Agent>) {
    if (currentUser.value) {
      currentUser.value = { ...currentUser.value, ...updates }
      persistAuth()
    }
  }

  function setPermissions(perms: string[]) {
    permissions.value = perms
  }

  function hasPerm(perm: string): boolean {
    return permissions.value.includes(perm)
  }

  function persistAuth() {
    if (typeof window !== 'undefined') {
      localStorage.setItem('auth_token', token.value ?? '')
      localStorage.setItem('auth_refresh', refreshToken.value ?? '')
      localStorage.setItem('auth_user', JSON.stringify(currentUser.value))
      localStorage.setItem('auth_perms', JSON.stringify(permissions.value))
    }
  }

  function restoreAuth() {
    if (typeof window === 'undefined') return false

    const savedToken = localStorage.getItem('auth_token')
    const savedUser = localStorage.getItem('auth_user')
    const savedPerms = localStorage.getItem('auth_perms')
    const savedRefresh = localStorage.getItem('auth_refresh')

    if (savedToken && savedUser) {
      token.value = savedToken
      refreshToken.value = savedRefresh
      currentUser.value = JSON.parse(savedUser)
      permissions.value = savedPerms ? JSON.parse(savedPerms) : []
      isAuthenticated.value = true
      return true
    }
    return false
  }

  function clearAuth() {
    if (typeof window !== 'undefined') {
      localStorage.removeItem('auth_token')
      localStorage.removeItem('auth_refresh')
      localStorage.removeItem('auth_user')
      localStorage.removeItem('auth_perms')
    }
  }

  return {
    // State
    currentUser,
    isAuthenticated,
    token,
    refreshToken,
    permissions,
    // Computed
    userDisplayName,
    userAvatar,
    userRole,
    hasPermission,
    // Actions
    login,
    logout,
    updateUser,
    setPermissions,
    hasPerm,
    restoreAuth,
  }
})
