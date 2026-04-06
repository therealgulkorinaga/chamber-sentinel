package com.chamber.sentinel.camera

import android.util.Log
import com.chamber.sentinel.ChamberRuntime
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

class FrameProcessor(
    private val runtime: ChamberRuntime,
    private val callback: EventCallback
) {
    interface EventCallback {
        fun onFrameProcessed(worldId: String, objectId: String)
        fun onEventSealed(eventType: String, timestamp: String)
        fun onError(error: String)
    }

    private val executor: ExecutorService = Executors.newSingleThreadExecutor { r ->
        Thread(r, "FrameProcessor").apply { isDaemon = true }
    }

    private val processing = AtomicBoolean(false)
    private val frameCount = AtomicLong(0)
    private val dropCount = AtomicLong(0)

    companion object {
        private const val TAG = "FrameProcessor"
    }

    fun processFrame(worldId: String, data: ByteArray, width: Int, height: Int, timestamp: Long) {
        if (!processing.compareAndSet(false, true)) {
            dropCount.incrementAndGet()
            return
        }

        executor.submit {
            try {
                val result = runtime.ingestFrame(worldId, data, width, height, timestamp)
                if (result != null) {
                    frameCount.incrementAndGet()
                    callback.onFrameProcessed(worldId, result)

                    if (frameCount.get() % 30 == 0L) {
                        Log.d(TAG, "Frames: ${frameCount.get()}, dropped: ${dropCount.get()}")
                    }
                } else {
                    callback.onError("Frame ingestion returned null")
                }
            } catch (e: Exception) {
                callback.onError("Frame processing failed: ${e.message}")
            } finally {
                processing.set(false)
            }
        }
    }

    fun shutdown() {
        executor.shutdownNow()
    }
}
