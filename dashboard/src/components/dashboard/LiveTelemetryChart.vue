<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useTelemetryStore } from '@/stores/telemetry'
import { Activity, Radio, Clock, Zap, RefreshCw } from 'lucide-vue-next'

const telemetryStore = useTelemetryStore()
const canvasRef = ref<HTMLCanvasElement | null>(null)
let animationFrameId: number | null = null

// Canvas 繪圖渲染函式
function renderWaveform() {
  const canvas = canvasRef.value
  if (!canvas) return

  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const dpr = window.devicePixelRatio || 1
  const width = canvas.clientWidth
  const height = canvas.clientHeight

  if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
    canvas.width = width * dpr
    canvas.height = height * dpr
  }

  ctx.save()
  ctx.scale(dpr, dpr)
  ctx.clearRect(0, 0, width, height)

  const points = telemetryStore.wavePoints
  const count = points.length
  if (count < 2) {
    ctx.restore()
    return
  }

  const stepX = width / (count - 1)
  const isDark = document.documentElement.classList.contains('dark')

  // 背景微網格線
  ctx.strokeStyle = isDark ? 'rgba(255, 255, 255, 0.04)' : 'rgba(0, 0, 0, 0.04)'
  ctx.lineWidth = 1
  for (let y = 20; y < height; y += 25) {
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(width, y)
    ctx.stroke()
  }

  // 計算座標點 (Y 軸反轉: 100 在最上方，0 在最下方，留有 padding)
  const coords = points.map((p, i) => {
    const x = i * stepX
    const y = height - (p / 100) * (height - 24) - 12
    return { x, y }
  })

  // 1. 繪製底部漸變填充
  const areaGradient = ctx.createLinearGradient(0, 0, 0, height)
  if (isDark) {
    areaGradient.addColorStop(0, 'rgba(16, 185, 129, 0.28)')
    areaGradient.addColorStop(1, 'rgba(16, 185, 129, 0.00)')
  } else {
    areaGradient.addColorStop(0, 'rgba(16, 185, 129, 0.20)')
    areaGradient.addColorStop(1, 'rgba(16, 185, 129, 0.00)')
  }

  ctx.beginPath()
  ctx.moveTo(coords[0].x, coords[0].y)
  for (let i = 0; i < coords.length - 1; i++) {
    const cpX = (coords[i].x + coords[i + 1].x) / 2
    ctx.quadraticCurveTo(coords[i].x, coords[i].y, cpX, (coords[i].y + coords[i + 1].y) / 2)
  }
  ctx.lineTo(coords[coords.length - 1].x, coords[coords.length - 1].y)
  ctx.lineTo(width, height)
  ctx.lineTo(0, height)
  ctx.closePath()
  ctx.fillStyle = areaGradient
  ctx.fill()

  // 2. 繪製平滑脈衝折線
  ctx.beginPath()
  ctx.moveTo(coords[0].x, coords[0].y)
  for (let i = 0; i < coords.length - 1; i++) {
    const cpX = (coords[i].x + coords[i + 1].x) / 2
    ctx.quadraticCurveTo(coords[i].x, coords[i].y, cpX, (coords[i].y + coords[i + 1].y) / 2)
  }
  ctx.lineTo(coords[coords.length - 1].x, coords[coords.length - 1].y)
  ctx.strokeStyle = isDark ? '#10b981' : '#059669'
  ctx.lineWidth = 2.5
  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'
  ctx.shadowColor = isDark ? 'rgba(16, 185, 129, 0.6)' : 'rgba(5, 150, 105, 0.3)'
  ctx.shadowBlur = 8
  ctx.stroke()

  // 3. 繪製最新點發光脈動
  const last = coords[coords.length - 1]
  ctx.shadowBlur = 0
  
  // 外擴光圈
  ctx.beginPath()
  ctx.arc(last.x, last.y, 6, 0, Math.PI * 2)
  ctx.fillStyle = isDark ? 'rgba(16, 185, 129, 0.35)' : 'rgba(5, 150, 105, 0.25)'
  ctx.fill()

  // 核心實心點
  ctx.beginPath()
  ctx.arc(last.x, last.y, 3.5, 0, Math.PI * 2)
  ctx.fillStyle = isDark ? '#34d399' : '#10b981'
  ctx.fill()

  ctx.restore()
}

// 60FPS 渲染循環
function startAnimationLoop() {
  function loop() {
    renderWaveform()
    animationFrameId = requestAnimationFrame(loop)
  }
  animationFrameId = requestAnimationFrame(loop)
}

onMounted(() => {
  telemetryStore.startTelemetryTicker()
  startAnimationLoop()
})

onUnmounted(() => {
  if (animationFrameId !== null) {
    cancelAnimationFrame(animationFrameId)
    animationFrameId = null
  }
})
</script>

<template>
  <div class="card p-5 bg-gradient-to-br from-white to-gray-50/60 dark:from-gray-800 dark:to-gray-900/60 border border-gray-200/80 dark:border-gray-700/80 shadow-sm relative overflow-hidden">
    <!-- 背景動態科技感氛圍光暈 -->
    <div class="absolute -right-16 -top-16 w-48 h-48 bg-emerald-500/10 rounded-full blur-3xl pointer-events-none" />

    <!-- 頂部標題與即時操作列 -->
    <div class="flex flex-wrap items-center justify-between gap-3 mb-4">
      <div class="flex items-center gap-2.5">
        <div class="p-2 bg-emerald-100 dark:bg-emerald-900/40 rounded-lg text-emerald-600 dark:text-emerald-400">
          <Activity class="w-5 h-5 animate-pulse" />
        </div>
        <div>
          <h2 class="text-base font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            系統即時動能與心跳波形
            <span class="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300">
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-ping mr-1.5" />
              Live 60 FPS
            </span>
          </h2>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            即時監控 Orchestrator WebSocket 脈衝波形與蜂群連線活躍度
          </p>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <button
          type="button"
          class="btn btn-secondary text-xs py-1.5 px-3 flex items-center gap-1.5 hover:border-emerald-500 hover:text-emerald-600 dark:hover:text-emerald-400 transition-colors"
          title="立即向 Port 8080 發送即時心跳探針"
          @click="telemetryStore.triggerPingProbe()"
        >
          <Zap class="w-3.5 h-3.5 text-amber-500" />
          發送心跳探針 (Ping)
        </button>
      </div>
    </div>

    <!-- 4 個即時遙測核心數據徽章 -->
    <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-4">
      <div class="bg-gray-50 dark:bg-gray-800/60 p-2.5 rounded-lg border border-gray-100 dark:border-gray-700/50 flex items-center gap-2.5">
        <Clock class="w-4 h-4 text-primary-500 shrink-0" />
        <div>
          <div class="text-[11px] text-gray-500 dark:text-gray-400">系統稼動時間</div>
          <div class="text-sm font-semibold font-mono text-gray-800 dark:text-gray-200">
            {{ telemetryStore.formattedUptime }}
          </div>
        </div>
      </div>

      <div class="bg-gray-50 dark:bg-gray-800/60 p-2.5 rounded-lg border border-gray-100 dark:border-gray-700/50 flex items-center gap-2.5">
        <Radio class="w-4 h-4 text-emerald-500 shrink-0" />
        <div>
          <div class="text-[11px] text-gray-500 dark:text-gray-400">連線延遲 (RTT)</div>
          <div class="text-sm font-semibold font-mono text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
            {{ telemetryStore.latencyMs }} ms
            <span class="text-[10px] text-gray-400 font-normal">極速</span>
          </div>
        </div>
      </div>

      <div class="bg-gray-50 dark:bg-gray-800/60 p-2.5 rounded-lg border border-gray-100 dark:border-gray-700/50 flex items-center gap-2.5">
        <Activity class="w-4 h-4 text-blue-500 shrink-0" />
        <div>
          <div class="text-[11px] text-gray-500 dark:text-gray-400">脈衝頻率</div>
          <div class="text-sm font-semibold font-mono text-gray-800 dark:text-gray-200">
            0.33 Hz <span class="text-[10px] text-gray-400 font-normal">(3s/次)</span>
          </div>
        </div>
      </div>

      <div class="bg-gray-50 dark:bg-gray-800/60 p-2.5 rounded-lg border border-gray-100 dark:border-gray-700/50 flex items-center gap-2.5">
        <RefreshCw class="w-4 h-4 text-amber-500 shrink-0" />
        <div>
          <div class="text-[11px] text-gray-500 dark:text-gray-400">累計脈衝數</div>
          <div class="text-sm font-semibold font-mono text-amber-600 dark:text-amber-400">
            # {{ telemetryStore.pulseCount }}
          </div>
        </div>
      </div>
    </div>

    <!-- 即時波形圖繪製畫布 -->
    <div class="relative w-full h-28 bg-gray-900/5 dark:bg-black/30 rounded-lg overflow-hidden border border-gray-200/50 dark:border-gray-800">
      <canvas
        ref="canvasRef"
        class="w-full h-full block"
      />
      <div class="absolute bottom-1 right-2 text-[10px] text-gray-400 dark:text-gray-500 font-mono select-none pointer-events-none">
        Telemetry Window: 35s
      </div>
    </div>
  </div>
</template>
