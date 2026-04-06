package com.chamber.sentinel.camera

import android.util.Log
import com.chamber.sentinel.ChamberRuntime
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

/**
 * Pipeline that receives raw camera frames from [CameraController]
 * and submits them into the Rust substrate via [ChamberRuntime].
 *
 * Frames are processed on a dedicated single-thread executor to avoid
 * blocking the camera callback thread. Back-pressure is handled by
 * dropping frames when the pipeline is busy.
 */
class FrameProcessor(
    private val runtime: ChamberRuntime,
    private val worldId: String,
) : CameraController.FrameCallback {

    private val executor: ExecutorService = Executors.newSingleThreadExecutor { r ->
        Thread(r, "FrameProcessor").apply { isDaemon = true }
    }

    private val processing = AtomicBoolean(false)
    private val frameCount = AtomicLong(0)
    private val dropCount = AtomicLong(0)

    /**
     * Called by [CameraController] when a new frame is available.
     * If the pipeline is already processing a frame, this one is dropped.
     */
    override fun onFrame(data: ByteArray, width: Int, height: Int, timestampNs: Long) {
        if (!processing.compareAndSet(false, true)) {
            dropCount.incrementAndGet()
            return
        }

        executor.execute {
            try {
                val timestampMs = timestampNs / 1_000_000
                runtime.ingestFrame(worldId, data, width, height, timestampMs)
                val count = frameCount.incrementAndGet()
                if (count % 100 == 0L) {
                    Log.d(
                        TAG,
                        "Processed $count frames, dropped ${dropCount.get()} " +
                                "(${width}x${height})"
                    )
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to submit frame to substrate", e)
            } finally {
                processing.set(false)
            }
        }
    }

    /**
     * Shut down the processing pipeline. Waits up to 1 second for pending work.
     */
    fun shutdown() {
        executor.shutdown()
        Log.i(
            TAG,
            "FrameProcessor shutdown: processed=${frameCount.get()}, dropped=${dropCount.get()}"
        )
    }

    /** Total frames successfully submitted to the substrate. */
    val processedFrames: Long get() = frameCount.get()

    /** Total frames dropped due to back-pressure. */
    val droppedFrames: Long get() = dropCount.get()

    companion object {
        private const val TAG = "FrameProcessor"
    }
}
