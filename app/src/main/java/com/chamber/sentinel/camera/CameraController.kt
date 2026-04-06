package com.chamber.sentinel.camera

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.TotalCaptureResult
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.util.Log
import android.util.Size
import androidx.core.content.ContextCompat

/**
 * Manages Camera2 API lifecycle for headless frame capture (no preview surface).
 *
 * Opens the rear camera, configures an [ImageReader] for JPEG capture,
 * and delivers frames to the registered [FrameCallback].
 */
class CameraController(private val context: Context) {

    fun interface FrameCallback {
        fun onFrame(data: ByteArray, width: Int, height: Int, timestampNs: Long)
    }

    private val cameraManager: CameraManager =
        context.getSystemService(Context.CAMERA_SERVICE) as CameraManager

    private var cameraDevice: CameraDevice? = null
    private var captureSession: CameraCaptureSession? = null
    private var imageReader: ImageReader? = null

    private var backgroundThread: HandlerThread? = null
    private var backgroundHandler: Handler? = null

    private var frameCallback: FrameCallback? = null
    private var targetSize: Size = Size(1920, 1080)

    /**
     * Set the callback that receives decoded frames.
     */
    fun setFrameCallback(callback: FrameCallback) {
        this.frameCallback = callback
    }

    /**
     * Set the desired capture resolution. Must be called before [open].
     */
    fun setTargetSize(width: Int, height: Int) {
        targetSize = Size(width, height)
    }

    /**
     * Open the camera and start the capture session.
     * Requires [Manifest.permission.CAMERA] to be granted.
     */
    fun open() {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA)
            != PackageManager.PERMISSION_GRANTED
        ) {
            Log.e(TAG, "CAMERA permission not granted")
            return
        }

        startBackgroundThread()

        val cameraId = findRearCamera() ?: run {
            Log.e(TAG, "No rear-facing camera found")
            return
        }

        try {
            cameraManager.openCamera(cameraId, stateCallback, backgroundHandler)
        } catch (e: SecurityException) {
            Log.e(TAG, "SecurityException opening camera", e)
        }
    }

    /**
     * Close the camera, release all resources.
     */
    fun close() {
        try {
            captureSession?.close()
            captureSession = null

            cameraDevice?.close()
            cameraDevice = null

            imageReader?.close()
            imageReader = null
        } finally {
            stopBackgroundThread()
        }
    }

    // -----------------------------------------------------------------------
    // Camera state callback
    // -----------------------------------------------------------------------

    private val stateCallback = object : CameraDevice.StateCallback() {
        override fun onOpened(camera: CameraDevice) {
            Log.i(TAG, "Camera opened: ${camera.id}")
            cameraDevice = camera
            createCaptureSession(camera)
        }

        override fun onDisconnected(camera: CameraDevice) {
            Log.w(TAG, "Camera disconnected")
            camera.close()
            cameraDevice = null
        }

        override fun onError(camera: CameraDevice, error: Int) {
            Log.e(TAG, "Camera error: $error")
            camera.close()
            cameraDevice = null
        }
    }

    // -----------------------------------------------------------------------
    // Capture session setup
    // -----------------------------------------------------------------------

    private fun createCaptureSession(camera: CameraDevice) {
        val reader = ImageReader.newInstance(
            targetSize.width,
            targetSize.height,
            ImageFormat.JPEG,
            /* maxImages = */ 2
        )

        reader.setOnImageAvailableListener({ ir ->
            val image = ir.acquireLatestImage() ?: return@setOnImageAvailableListener
            try {
                val buffer = image.planes[0].buffer
                val bytes = ByteArray(buffer.remaining())
                buffer.get(bytes)
                frameCallback?.onFrame(
                    bytes,
                    image.width,
                    image.height,
                    image.timestamp
                )
            } finally {
                image.close()
            }
        }, backgroundHandler)

        imageReader = reader

        try {
            camera.createCaptureSession(
                listOf(reader.surface),
                object : CameraCaptureSession.StateCallback() {
                    override fun onConfigured(session: CameraCaptureSession) {
                        Log.i(TAG, "Capture session configured")
                        captureSession = session
                        startRepeatingCapture(session, camera)
                    }

                    override fun onConfigureFailed(session: CameraCaptureSession) {
                        Log.e(TAG, "Capture session configuration failed")
                    }
                },
                backgroundHandler
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create capture session", e)
        }
    }

    private fun startRepeatingCapture(session: CameraCaptureSession, camera: CameraDevice) {
        try {
            val builder = camera.createCaptureRequest(CameraDevice.TEMPLATE_STILL_CAPTURE)
            imageReader?.surface?.let { builder.addTarget(it) }

            // Auto-focus and auto-exposure
            builder.set(
                CaptureRequest.CONTROL_AF_MODE,
                CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE
            )
            builder.set(
                CaptureRequest.CONTROL_AE_MODE,
                CaptureRequest.CONTROL_AE_MODE_ON
            )

            session.setRepeatingRequest(
                builder.build(),
                object : CameraCaptureSession.CaptureCallback() {
                    override fun onCaptureCompleted(
                        session: CameraCaptureSession,
                        request: CaptureRequest,
                        result: TotalCaptureResult
                    ) {
                        // Frame delivered via ImageReader callback
                    }
                },
                backgroundHandler
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start repeating capture", e)
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private fun findRearCamera(): String? {
        for (id in cameraManager.cameraIdList) {
            val characteristics = cameraManager.getCameraCharacteristics(id)
            val facing = characteristics.get(CameraCharacteristics.LENS_FACING)
            if (facing == CameraCharacteristics.LENS_FACING_BACK) {
                return id
            }
        }
        return null
    }

    private fun startBackgroundThread() {
        backgroundThread = HandlerThread("CameraBackground").also { it.start() }
        backgroundHandler = Handler(backgroundThread!!.looper)
    }

    private fun stopBackgroundThread() {
        backgroundThread?.quitSafely()
        try {
            backgroundThread?.join()
        } catch (_: InterruptedException) {
            // ignored
        }
        backgroundThread = null
        backgroundHandler = null
    }

    companion object {
        private const val TAG = "CameraController"
    }
}
