package com.chamber.sentinel.integrity

import android.app.Activity
import android.content.Context
import android.hardware.display.DisplayManager
import android.util.Log
import android.view.Display

/**
 * Detects active screen recording / casting by monitoring virtual displays
 * through the [DisplayManager] API.
 *
 * Virtual displays are created when screen mirroring, casting, or
 * MediaProjection-based screen recording is active. Their presence
 * while Chamber Sentinel is running constitutes a violation.
 */
class ScreenCaptureMonitor(
    private val context: Context,
    private val violationListener: IntegrityMonitor.ViolationListener,
) {

    private val displayManager: DisplayManager =
        context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager

    private var registered = false

    private val displayListener = object : DisplayManager.DisplayListener {

        override fun onDisplayAdded(displayId: Int) {
            val display = displayManager.getDisplay(displayId) ?: return
            checkDisplay(display, "added")
        }

        override fun onDisplayChanged(displayId: Int) {
            val display = displayManager.getDisplay(displayId) ?: return
            checkDisplay(display, "changed")
        }

        override fun onDisplayRemoved(displayId: Int) {
            Log.d(TAG, "Display removed: $displayId")
        }
    }

    /**
     * Start monitoring for virtual displays.
     */
    fun start() {
        // Check existing displays for virtual ones
        for (display in displayManager.displays) {
            checkDisplay(display, "existing")
        }

        displayManager.registerDisplayListener(displayListener, null)
        registered = true
        Log.i(TAG, "ScreenCaptureMonitor started")
    }

    /**
     * Stop monitoring.
     */
    fun stop() {
        if (registered) {
            displayManager.unregisterDisplayListener(displayListener)
            registered = false
        }
        Log.i(TAG, "ScreenCaptureMonitor stopped")
    }

    private fun checkDisplay(display: Display, action: String) {
        // Virtual displays have flag FLAG_PRESENTATION or are not the default display
        // and have FLAG_PRIVATE cleared. Overlay and virtual displays used for
        // screen recording typically show up as non-default, non-presentation displays.
        val flags = display.flags
        val isVirtual = (flags and Display.FLAG_PRIVATE) == 0 &&
                display.displayId != Display.DEFAULT_DISPLAY

        if (isVirtual) {
            violationListener.onViolation(
                "ScreenCaptureMonitor",
                "Virtual display $action: id=${display.displayId}, " +
                        "name='${display.name}', flags=0x${flags.toString(16)}. " +
                        "Possible screen recording or casting."
            )
        }

        Log.d(TAG, "Display $action: id=${display.displayId}, name='${display.name}', " +
                "flags=0x${flags.toString(16)}")
    }

    companion object {
        private const val TAG = "ScreenCaptureMonitor"
    }
}
