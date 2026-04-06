package com.chamber.sentinel.integrity

import android.content.Context
import android.hardware.camera2.CameraManager
import android.util.Log

/**
 * Monitors camera availability changes via [CameraManager.AvailabilityCallback].
 *
 * Detects when another application opens a camera while Chamber Sentinel
 * expects exclusive access, which may indicate a surveillance attempt.
 */
class CameraAccessMonitor(
    context: Context,
    private val violationListener: IntegrityMonitor.ViolationListener,
) {

    private val cameraManager: CameraManager =
        context.getSystemService(Context.CAMERA_SERVICE) as CameraManager

    /** Track which cameras we believe should be available. */
    private val expectedAvailable = mutableSetOf<String>()

    /** Whether we are currently the ones using the camera. */
    @Volatile
    var ownCameraActive: Boolean = false

    private val availabilityCallback = object : CameraManager.AvailabilityCallback() {

        override fun onCameraAvailable(cameraId: String) {
            Log.d(TAG, "Camera $cameraId became available")
            expectedAvailable.add(cameraId)
        }

        override fun onCameraUnavailable(cameraId: String) {
            Log.d(TAG, "Camera $cameraId became unavailable")

            if (!ownCameraActive && expectedAvailable.contains(cameraId)) {
                // Another process grabbed the camera while we weren't using it
                violationListener.onViolation(
                    "CameraAccessMonitor",
                    "Camera $cameraId became unavailable while not in use by Sentinel. " +
                            "Another process may be accessing the camera."
                )
            }

            expectedAvailable.remove(cameraId)
        }

        override fun onCameraAccessPrioritiesChanged() {
            Log.d(TAG, "Camera access priorities changed")
            violationListener.onViolation(
                "CameraAccessMonitor",
                "Camera access priorities changed — another high-priority camera " +
                        "client may be active"
            )
        }
    }

    /**
     * Register the availability callback with the system camera service.
     */
    fun start() {
        // Populate initial state
        for (id in cameraManager.cameraIdList) {
            expectedAvailable.add(id)
        }

        cameraManager.registerAvailabilityCallback(availabilityCallback, null)
        Log.i(TAG, "CameraAccessMonitor started, tracking ${expectedAvailable.size} cameras")
    }

    /**
     * Unregister the callback.
     */
    fun stop() {
        cameraManager.unregisterAvailabilityCallback(availabilityCallback)
        expectedAvailable.clear()
        Log.i(TAG, "CameraAccessMonitor stopped")
    }

    companion object {
        private const val TAG = "CameraAccessMonitor"
    }
}
