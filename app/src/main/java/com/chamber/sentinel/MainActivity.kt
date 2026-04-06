package com.chamber.sentinel

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.WindowManager
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.chamber.sentinel.camera.CameraController
import com.chamber.sentinel.camera.FrameProcessor
import com.chamber.sentinel.ui.EventListFragment

class MainActivity : AppCompatActivity() {

    private lateinit var runtime: ChamberRuntime
    private var cameraController: CameraController? = null
    private var frameProcessor: FrameProcessor? = null
    private var detector: com.chamber.sentinel.detection.ObjectDetector? = null
    private var currentWorldId: String? = null
    private val handler = Handler(Looper.getMainLooper())
    private var frameCount = 0
    private var lastFrameBytes: ByteArray? = null // Keep last frame for detection before burn
    private val CHAMBER_WINDOW_SIZE = 30 // Burn every 30 frames (1 second at 30fps)

    companion object {
        private const val TAG = "MainActivity"
        private const val CAMERA_PERMISSION_REQUEST = 100
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE
        )

        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        WindowCompat.setDecorFitsSystemWindows(window, false)
        val controller = WindowInsetsControllerCompat(window, window.decorView)
        controller.hide(WindowInsetsCompat.Type.systemBars())
        controller.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE

        runtime = ChamberRuntime.getInstance()
        runtime.initialize()
        Log.i(TAG, "Runtime initialized: version=${runtime.version}")

        if (savedInstanceState == null) {
            supportFragmentManager.beginTransaction()
                .replace(R.id.fragment_container, EventListFragment())
                .commit()
        }

        // Camera will start in onResume after the activity is fully visible
    }

    private var cameraStarted = false

    private fun checkCameraPermissionAndStart() {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA)
            == PackageManager.PERMISSION_GRANTED) {
            if (!cameraStarted) {
                // Delay slightly to ensure we're in foreground state
                handler.postDelayed({ startCameraProcessing() }, 500)
            }
        } else {
            ActivityCompat.requestPermissions(
                this, arrayOf(Manifest.permission.CAMERA), CAMERA_PERMISSION_REQUEST)
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int, permissions: Array<out String>, grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == CAMERA_PERMISSION_REQUEST &&
            grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
            startCameraProcessing()
        } else {
            Log.w(TAG, "Camera permission denied — running without camera")
        }
    }

    private fun startCameraProcessing() {
        if (cameraStarted) return
        cameraStarted = true
        Log.i(TAG, "Starting camera processing")

        // Create first chamber
        currentWorldId = runtime.createWorld("camera_sentinel_v1", "Camera monitoring")
        Log.i(TAG, "First chamber created: $currentWorldId")
        frameCount = 0

        // Initialize object detector
        try {
            detector = com.chamber.sentinel.detection.ObjectDetector(this)
            Log.i(TAG, "Object detector loaded")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to load detector: ${e.message}")
        }

        // Set up frame processor
        frameProcessor = FrameProcessor(runtime, object : FrameProcessor.EventCallback {
            override fun onFrameProcessed(worldId: String, objectId: String) {
                frameCount++
                // Rolling chamber: burn and create new every CHAMBER_WINDOW_SIZE frames
                if (frameCount >= CHAMBER_WINDOW_SIZE) {
                    rollChamber()
                }
            }

            override fun onEventSealed(eventType: String, timestamp: String) {
                Log.i(TAG, "Event sealed: $eventType at $timestamp")
                handler.post {
                    val fragment = supportFragmentManager
                        .findFragmentById(R.id.fragment_container) as? EventListFragment
                    fragment?.addEvent(AuditEvent(
                        worldId = currentWorldId ?: "",
                        timestamp = timestamp,
                        eventType = eventType,
                        detail = ""
                    ))
                }
            }

            override fun onError(error: String) {
                Log.e(TAG, "Frame processing error: $error")
            }
        })

        // Start camera
        try {
            cameraController = CameraController(this).apply {
                setFrameCallback { data, width, height, timestamp ->
                    val wid = currentWorldId ?: return@setFrameCallback
                    frameProcessor?.processFrame(wid, data, width, height, timestamp)
                    lastFrameBytes = data.clone()                }
                open()
            }
            Log.i(TAG, "Camera started — processing frames into chambers")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start camera: ${e.message}")
            cameraStarted = false
        }
    }

    private fun rollChamber() {
        val oldWorldId = currentWorldId ?: return
        val now = System.currentTimeMillis()
        val timestampStr = java.text.SimpleDateFormat("HH:mm:ss", java.util.Locale.US).format(java.util.Date(now))

        // Run detection on the last frame before burning
        var eventType = "motion_detected"
        var confidence = 0.5f
        val frameData = lastFrameBytes
        if (frameData != null && detector != null) {
            try {
                val detections = detector!!.detect(frameData)
                if (detections.isNotEmpty()) {
                    val best = detections.maxByOrNull { it.confidence }!!
                    eventType = best.label
                    confidence = best.confidence
                }
            } catch (e: Exception) {
                Log.w(TAG, "Detection failed: ${e.message}")
            }
            frameData.fill(0) // Zero the frame — analyzed, now forget
            lastFrameBytes = null
        }

        // Seal the detected event
        val eventJson = """{"event_type":"$eventType","timestamp":"$now","confidence":$confidence,"duration_seconds":1}"""
        try {
            val eventId = runtime.createObject(oldWorldId, "event_summary", eventJson)
            runtime.sealArtifact(oldWorldId, eventId)

            val confidenceStr = "%.0f%%".format(confidence * 100)
            val displayType = eventType.replace("_", " ")
            handler.post {
                val fragment = supportFragmentManager
                    .findFragmentById(R.id.fragment_container) as? EventListFragment
                fragment?.addEvent(AuditEvent(
                    worldId = oldWorldId,
                    timestamp = timestampStr,
                    eventType = "$displayType ($confidenceStr)",
                    detail = "${frameCount} frames burned"
                ))
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to seal event: ${e.message}")
        }

        // Burn the old chamber
        try {
            runtime.burn(oldWorldId, "AutoBurn")
            Log.i(TAG, "Chamber burned")
        } catch (e: Exception) {
            Log.e(TAG, "Burn failed: ${e.message}")
        }

        // Create new chamber
        currentWorldId = runtime.createWorld("camera_sentinel_v1", "Camera monitoring")
        frameCount = 0
        Log.i(TAG, "New chamber: $currentWorldId")
    }

    override fun onResume() {
        super.onResume()
        // Don't start camera here — use onWindowFocusChanged instead
        // which fires after the window is fully visible and the process
        // is in foreground state
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            checkCameraPermissionAndStart()
        }
    }

    override fun onPause() {
        super.onPause()
        cameraController?.close()
    }

    override fun onDestroy() {
        super.onDestroy()
        cameraController?.close()
        // Burn any active chamber
        currentWorldId?.let { wid ->
            runtime.burn(wid, "AutoBurn")
            Log.i(TAG, "Final chamber burned on destroy")
        }
        if (isFinishing) {
            runtime.destroy()
        }
    }
}
