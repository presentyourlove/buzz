<script setup lang="ts">
import { PageHeader } from '@/components'
</script>

<template>
  <div class="p-6">
    <PageHeader
      title="系統設定"
      subtitle="管理面板配置、WebSocket 連線、主題與通知偏好"
      :breadcrumbs="[
        { label: '首頁', href: '/' },
        { label: '系統設定' },
      ]"
    />
    <div class="card p-8 text-center">
      <p class="text-gray-500 dark:text-gray-400">設定頁面待實作</p>
    </div>
  </div>
</template>
