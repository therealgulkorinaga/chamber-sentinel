package com.chamber.sentinel.integrity

import android.content.Context
import android.net.TrafficStats
import android.util.Log
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

/**
 * Monitors network traffic using [TrafficStats] to detect unexpected data exfiltration.
 *
 * Since Chamber Sentinel explicitly removes INTERNET permission, any observed
 * TX bytes indicates a violation — either a permission bypass or a rogue component.
 *
 * Polls every 500ms on a dedicated scheduled executor.
 */
class NetworkMonitor(
    private val context: Context,
    private val violationListener: IntegrityMonitor.ViolationListener,
) {

    private var executor: ScheduledExecutorService? = null
    private var pollFuture: ScheduledFuture<*>? = null

    private var baselineTxBytes: Long = 0L
    private var baselineRxBytes: Long = 0L
    private var lastTxBytes: Long = 0L
    private var lastRxBytes: Long = 0L

    /**
     * Start polling TrafficStats every [POLL_INTERVAL_MS] milliseconds.
     */
    fun start() {
        val uid = context.applicationInfo.uid

        baselineTxBytes = TrafficStats.getUidTxBytes(uid)
        baselineRxBytes = TrafficStats.getUidRxBytes(uid)
        lastTxBytes = baselineTxBytes
        lastRxBytes = baselineRxBytes

        Log.i(TAG, "NetworkMonitor started for UID $uid " +
                "(baseline TX=$baselineTxBytes, RX=$baselineRxBytes)")

        executor = Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "NetworkMonitor").apply { isDaemon = true }
        }

        pollFuture = executor?.scheduleAtFixedRate(
            { pollTrafficStats(uid) },
            POLL_INTERVAL_MS,
            POLL_INTERVAL_MS,
            TimeUnit.MILLISECONDS
        )
    }

    /**
     * Stop polling.
     */
    fun stop() {
        pollFuture?.cancel(false)
        pollFuture = null
        executor?.shutdownNow()
        executor = null
        Log.i(TAG, "NetworkMonitor stopped")
    }

    private fun pollTrafficStats(uid: Int) {
        try {
            val currentTx = TrafficStats.getUidTxBytes(uid)
            val currentRx = TrafficStats.getUidRxBytes(uid)

            val deltaTx = currentTx - lastTxBytes
            val deltaRx = currentRx - lastRxBytes

            if (deltaTx > TX_THRESHOLD_BYTES) {
                violationListener.onViolation(
                    "NetworkMonitor",
                    "Unexpected TX traffic detected: $deltaTx bytes " +
                            "(total since baseline: ${currentTx - baselineTxBytes})"
                )
            }

            if (deltaRx > RX_THRESHOLD_BYTES) {
                violationListener.onViolation(
                    "NetworkMonitor",
                    "Unexpected RX traffic detected: $deltaRx bytes " +
                            "(total since baseline: ${currentRx - baselineRxBytes})"
                )
            }

            lastTxBytes = currentTx
            lastRxBytes = currentRx
        } catch (e: Exception) {
            Log.e(TAG, "Error polling TrafficStats", e)
        }
    }

    companion object {
        private const val TAG = "NetworkMonitor"
        private const val POLL_INTERVAL_MS = 500L

        // Any non-trivial traffic is suspicious since we have no INTERNET permission
        private const val TX_THRESHOLD_BYTES = 0L
        private const val RX_THRESHOLD_BYTES = 0L
    }
}
