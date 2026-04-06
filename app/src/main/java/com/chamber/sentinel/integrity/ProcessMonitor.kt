package com.chamber.sentinel.integrity

import android.os.Handler
import android.util.Log
import java.io.File

/**
 * Periodically scans the /proc filesystem to detect potentially hostile processes
 * or unexpected process count changes.
 *
 * Since Android's /proc access is restricted per-UID, this primarily monitors
 * our own process tree and watches for anomalies like:
 * - Unexpected child processes (injection)
 * - Debugger attachment (TracerPid != 0 in /proc/self/status)
 */
class ProcessMonitor(
    private val violationListener: IntegrityMonitor.ViolationListener,
) {

    private var handler: Handler? = null
    private var running = false

    private val pollRunnable = object : Runnable {
        override fun run() {
            if (!running) return
            checkProcessIntegrity()
            handler?.postDelayed(this, POLL_INTERVAL_MS)
        }
    }

    /**
     * Start periodic process scanning.
     */
    fun start(handler: Handler) {
        this.handler = handler
        this.running = true
        handler.post(pollRunnable)
        Log.i(TAG, "ProcessMonitor started (interval=${POLL_INTERVAL_MS}ms)")
    }

    /**
     * Stop scanning.
     */
    fun stop() {
        running = false
        handler?.removeCallbacks(pollRunnable)
        handler = null
        Log.i(TAG, "ProcessMonitor stopped")
    }

    private fun checkProcessIntegrity() {
        try {
            checkDebuggerAttached()
            checkChildProcesses()
        } catch (e: Exception) {
            Log.e(TAG, "Error during process integrity check", e)
        }
    }

    /**
     * Read /proc/self/status and check TracerPid. A non-zero value means
     * a debugger or ptrace is attached.
     */
    private fun checkDebuggerAttached() {
        val statusFile = File("/proc/self/status")
        if (!statusFile.canRead()) return

        val lines = statusFile.readLines()
        for (line in lines) {
            if (line.startsWith("TracerPid:")) {
                val tracerPid = line.substringAfter("TracerPid:").trim().toIntOrNull() ?: 0
                if (tracerPid != 0) {
                    violationListener.onViolation(
                        "ProcessMonitor",
                        "Debugger/ptrace detected: TracerPid=$tracerPid"
                    )
                }
                break
            }
        }
    }

    /**
     * Read /proc/self/task/ to count threads. A sudden spike might indicate
     * thread injection.
     */
    private fun checkChildProcesses() {
        val taskDir = File("/proc/self/task")
        if (!taskDir.isDirectory) return

        val threadCount = taskDir.listFiles()?.size ?: 0

        if (lastThreadCount > 0 && threadCount > lastThreadCount + THREAD_SPIKE_THRESHOLD) {
            violationListener.onViolation(
                "ProcessMonitor",
                "Thread count spike detected: $lastThreadCount -> $threadCount " +
                        "(+${threadCount - lastThreadCount})"
            )
        }

        lastThreadCount = threadCount
    }

    private var lastThreadCount = 0

    companion object {
        private const val TAG = "ProcessMonitor"
        private const val POLL_INTERVAL_MS = 2000L
        private const val THREAD_SPIKE_THRESHOLD = 5
    }
}
