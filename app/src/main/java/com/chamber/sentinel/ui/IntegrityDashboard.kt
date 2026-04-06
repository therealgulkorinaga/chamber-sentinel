package com.chamber.sentinel.ui

import android.os.Bundle
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView
import androidx.fragment.app.Fragment
import com.chamber.sentinel.R
import java.util.concurrent.atomic.AtomicInteger

/**
 * Fragment displaying the current integrity status of Chamber Sentinel.
 *
 * Shows:
 * - Overall status (SECURE / VIOLATION DETECTED)
 * - Per-monitor status indicators
 * - Violation count
 */
class IntegrityDashboard : Fragment() {

    private lateinit var statusText: TextView
    private lateinit var violationCountText: TextView
    private lateinit var networkStatus: TextView
    private lateinit var cameraStatus: TextView
    private lateinit var fileStatus: TextView
    private lateinit var processStatus: TextView
    private lateinit var screenStatus: TextView

    private val violationCount = AtomicInteger(0)

    private val monitorStates = mutableMapOf(
        "NetworkMonitor" to true,
        "CameraAccessMonitor" to true,
        "FileMonitor" to true,
        "ProcessMonitor" to true,
        "ScreenCaptureMonitor" to true,
    )

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        val layout = LinearLayout(requireContext()).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(0xFF0A0A0A.toInt())
            setPadding(48, 48, 48, 48)
            gravity = Gravity.TOP or Gravity.START
        }

        statusText = createHeaderText().also { layout.addView(it) }
        violationCountText = createSubText("Violations: 0").also { layout.addView(it) }

        // Spacer
        layout.addView(View(requireContext()).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 48
            )
        })

        // Monitor status indicators
        val monitorHeader = createSectionText("Monitor Status").also { layout.addView(it) }
        networkStatus = createMonitorRow("Network").also { layout.addView(it) }
        cameraStatus = createMonitorRow("Camera Access").also { layout.addView(it) }
        fileStatus = createMonitorRow("File System").also { layout.addView(it) }
        processStatus = createMonitorRow("Process").also { layout.addView(it) }
        screenStatus = createMonitorRow("Screen Capture").also { layout.addView(it) }

        updateDisplay()
        return layout
    }

    /**
     * Report a violation from a specific monitor.
     */
    fun reportViolation(source: String, description: String) {
        violationCount.incrementAndGet()
        monitorStates[source] = false

        if (isAdded) {
            requireActivity().runOnUiThread { updateDisplay() }
        }
    }

    /**
     * Reset all monitor states to secure.
     */
    fun reset() {
        violationCount.set(0)
        monitorStates.keys.forEach { monitorStates[it] = true }

        if (isAdded) {
            requireActivity().runOnUiThread { updateDisplay() }
        }
    }

    private fun updateDisplay() {
        val secure = monitorStates.values.all { it }
        val count = violationCount.get()

        statusText.text = if (secure) {
            getString(R.string.status_secure)
        } else {
            getString(R.string.status_violation)
        }
        statusText.setTextColor(
            if (secure) 0xFF00FF41.toInt() else 0xFFFF4444.toInt()
        )

        violationCountText.text = "Violations: $count"

        networkStatus.setTextColor(statusColor("NetworkMonitor"))
        cameraStatus.setTextColor(statusColor("CameraAccessMonitor"))
        fileStatus.setTextColor(statusColor("FileMonitor"))
        processStatus.setTextColor(statusColor("ProcessMonitor"))
        screenStatus.setTextColor(statusColor("ScreenCaptureMonitor"))
    }

    private fun statusColor(monitor: String): Int {
        return if (monitorStates[monitor] == true) 0xFF00FF41.toInt() else 0xFFFF4444.toInt()
    }

    // -----------------------------------------------------------------------
    // View factory helpers
    // -----------------------------------------------------------------------

    private fun createHeaderText(): TextView = TextView(requireContext()).apply {
        text = getString(R.string.status_secure)
        textSize = 32f
        setTextColor(0xFF00FF41.toInt())
        typeface = android.graphics.Typeface.create("monospace", android.graphics.Typeface.BOLD)
        layoutParams = LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        ).apply { bottomMargin = 16 }
    }

    private fun createSubText(label: String): TextView = TextView(requireContext()).apply {
        text = label
        textSize = 16f
        setTextColor(0xFFAAAAAA.toInt())
        typeface = android.graphics.Typeface.create("monospace", android.graphics.Typeface.NORMAL)
        layoutParams = LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        ).apply { bottomMargin = 8 }
    }

    private fun createSectionText(label: String): TextView = TextView(requireContext()).apply {
        text = label
        textSize = 18f
        setTextColor(0xFFCCCCCC.toInt())
        typeface = android.graphics.Typeface.create("monospace", android.graphics.Typeface.BOLD)
        layoutParams = LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        ).apply { bottomMargin = 16 }
    }

    private fun createMonitorRow(label: String): TextView = TextView(requireContext()).apply {
        text = "\u25CF $label"
        textSize = 14f
        setTextColor(0xFF00FF41.toInt())
        typeface = android.graphics.Typeface.create("monospace", android.graphics.Typeface.NORMAL)
        layoutParams = LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        ).apply {
            bottomMargin = 8
            leftMargin = 16
        }
    }
}
