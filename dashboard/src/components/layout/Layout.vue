<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from 'vue'
import { useAppStore } from '@/stores'
import { cn } from '@/utils'
import Sidebar from './Sidebar.vue'
import Header from './Header.vue'

const appStore = useAppStore()
const isMobile = ref(false)

const checkMobile = () => {
  isMobile.value = window.innerWidth < 1024
}

onMounted(() => {
  checkMobile()
  window.addEventListener('resize', checkMobile)
  appStore.initialize()
})

onUnmounted(() => {
  window.removeEventListener('resize', checkMobile)
})

const mainClasses = computed(() =>
  cn(
    'min-h-screen transition-all duration-300',
    {
      'pt-16': true,
      'pl-64': !appStore.sidebarCollapsed && !isMobile.value,
      'pl-16': appStore.sidebarCollapsed && !isMobile.value,
      'pl-0': isMobile.value,
    }
  )
)
</script>

<template>
  <div class="min-h-screen bg-gray-50 dark:bg-gray-900">
    <Sidebar />
    <Header />
    <main :class="mainClasses" role="main" aria-label="主要內容區域">
      <slot />
    </main>
  </div>
</template>
