import { format, formatDistanceToNow, formatDuration, parseISO } from 'date-fns'
import { zhTW } from 'date-fns/locale'

export function formatDate(date: string | Date, pattern = 'yyyy/MM/dd'): string {
  const d = typeof date === 'string' ? parseISO(date) : date
  return format(d, pattern, { locale: zhTW })
}

export function formatDateTime(date: string | Date, pattern = 'yyyy/MM/dd HH:mm'): string {
  const d = typeof date === 'string' ? parseISO(date) : date
  return format(d, pattern, { locale: zhTW })
}

export function formatRelativeTime(date: string | Date): string {
  const d = typeof date === 'string' ? parseISO(date) : date
  return formatDistanceToNow(d, { addSuffix: true, locale: zhTW })
}

export function formatDurationMs(ms: number): string {
  return formatDuration({ seconds: ms / 1000 }, { locale: zhTW })
}

export function isOverdue(date: string | Date): boolean {
  const d = typeof date === 'string' ? parseISO(date) : date
  return d < new Date()
}

export function isToday(date: string | Date): boolean {
  const d = typeof date === 'string' ? parseISO(date) : date
  const today = new Date()
  return d.getDate() === today.getDate() &&
    d.getMonth() === today.getMonth() &&
    d.getFullYear() === today.getFullYear()
}
