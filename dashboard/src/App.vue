<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { RouterView } from 'vue-router'
import Layout from '@/components/layout/Layout.vue'
import { useAgentStore } from '@/stores/agent'
import { useTaskStore } from '@/stores/task'

const agentStore = useAgentStore()
const taskStore = useTaskStore()

let taskTimer: ReturnType<typeof setInterval> | null = null

onMounted(() => {
  // 啟動特工雙軌即時更新（WebSocket 優先 + 5s 輪詢降級保護）
  agentStore.autoConnect()
  // 啟動任務工單載入與 10s 定時輪詢刷新
  taskStore.fetchTasks()
  taskTimer = setInterval(() => {
    taskStore.fetchTasks()
  }, 10000)
})

onUnmounted(() => {
  agentStore.stopPolling()
  agentStore.disconnectWS()
  if (taskTimer) {
    clearInterval(taskTimer)
    taskTimer = null
  }
})
</script>

<template>
  <Layout>
    <RouterView />
  </Layout>
</template>

<style scoped>
/* App-level styles if needed */
</style>
