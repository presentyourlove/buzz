<script setup lang="ts">
import { useRouter } from 'vue-router'
import { Home, ArrowLeft } from 'lucide-vue-next'

const router = useRouter()
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900">
    <div class="text-center p-8">
      <h1 class="text-9xl font-bold text-primary-500/20 dark:text-primary-500/10 mb-4">404</h1>
      <h2 class="text-2xl font-semibold text-gray-900 dark:text-gray-100 mb-2">找不到頁面</h2>
      <p class="text-gray-500 dark:text-gray-400 mb-6 max-w-md mx-auto">
        抱歉，您訪問的頁面不存在或已被移除。請檢查網址或返回首頁。
      </p>
      <div class="flex gap-4 justify-center">
        <button
          class="btn btn-primary"
          @click="router.push('/')"
        >
          <Home class="w-4 h-4 mr-2" />
          返回首頁
        </button>
        <button
          class="btn btn-secondary"
          @click="router.back()"
        >
          <ArrowLeft class="w-4 h-4 mr-2" />
          返回上一頁
        </button>
      </div>
    </div>
  </div>
</template>
