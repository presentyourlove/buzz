/**
 * System Health Guardian Skill
 * 核心功能：系統指標監控、熔斷保護機制 (Circuit Breaker) 與自愈重試
 */

import os from 'node:os';

export class CircuitBreaker {
  constructor(threshold = 5, timeout = 60000) {
    this.threshold = threshold;
    this.timeout = timeout;
    this.state = 'CLOSED'; // CLOSED, OPEN, HALF_OPEN
    this.failureCount = 0;
    this.lastFailureTime = null;
  }

  recordSuccess() {
    this.failureCount = 0;
    this.state = 'CLOSED';
  }

  recordFailure() {
    this.failureCount += 1;
    this.lastFailureTime = Date.now();
    if (this.failureCount >= this.threshold) {
      this.state = 'OPEN';
    }
  }

  canExecute() {
    if (this.state === 'CLOSED') return true;
    if (this.state === 'OPEN') {
      if (Date.now() - this.lastFailureTime > this.timeout) {
        this.state = 'HALF_OPEN';
        return true;
      }
      return false;
    }
    return true; // HALF_OPEN allows single probe
  }
}

export class SystemHealthGuardian {
  constructor(config = {}) {
    this.config = config;
    this.circuitBreaker = new CircuitBreaker(5, 30000);
  }

  checkMetrics() {
    const totalMem = os.totalmem();
    const freeMem = os.freemem();
    const memPercent = Math.round(((totalMem - freeMem) / totalMem) * 100);
    const cpuCount = os.cpus().length;
    const loadAvg = os.loadavg();

    const healthy = memPercent < 90;
    return {
      healthy,
      metrics: {
        memory_percent: memPercent,
        cpu_count: cpuCount,
        load_avg_1m: loadAvg[0]
      },
      circuit_state: this.circuitBreaker.state,
      timestamp: new Date().toISOString()
    };
  }
}

export default SystemHealthGuardian;
