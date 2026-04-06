package com.chamber.sentinel.integrity

import android.os.Environment
import android.os.FileObserver
import android.util.Log
import java.io.File

/**
 * Monitors filesystem paths commonly used for screenshot / screen recording output
 * using the Android [FileObserver] API.
 *
 * Watches:
 * - DCIM/ (camera screenshots)
 * - Pictures/ (screenshot saves)
 * - A temporary directory for transient captures
 *
 * Any file creation, modification, or move-to event triggers a violation.
 */
class FileMonitor(
    private val violationListener: IntegrityMonitor.ViolationListener,
) {

    private val observers = mutableListOf<FileObserver>()

    /**
     * Start watching the monitored directories.
     */
    fun start() {
        val watchDirs = listOf(
            File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DCIM), ""),
            File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_PICTURES), ""),
            File(Environment.getExternalStorageDirectory(), "tmp"),
        )

        for (dir in watchDirs) {
            if (!dir.exists()) {
                Log.d(TAG, "Watch directory does not exist, skipping: ${dir.absolutePath}")
                continue
            }

            val observer = object : FileObserver(dir, WATCH_MASK) {
                override fun onEvent(event: Int, path: String?) {
                    if (path == null) return
                    handleFileEvent(dir, event, path)
                }
            }

            observer.startWatching()
            observers.add(observer)
            Log.i(TAG, "Watching directory: ${dir.absolutePath}")
        }

        Log.i(TAG, "FileMonitor started with ${observers.size} observers")
    }

    /**
     * Stop all file observers.
     */
    fun stop() {
        for (observer in observers) {
            observer.stopWatching()
        }
        observers.clear()
        Log.i(TAG, "FileMonitor stopped")
    }

    private fun handleFileEvent(dir: File, event: Int, path: String) {
        val eventName = when (event and ALL_EVENTS_MASK) {
            CREATE -> "CREATE"
            MODIFY -> "MODIFY"
            MOVED_TO -> "MOVED_TO"
            CLOSE_WRITE -> "CLOSE_WRITE"
            else -> "EVENT(${event and ALL_EVENTS_MASK})"
        }

        // Check if the file looks like a screenshot or recording
        val lowerPath = path.lowercase()
        val suspicious = lowerPath.endsWith(".png") ||
                lowerPath.endsWith(".jpg") ||
                lowerPath.endsWith(".jpeg") ||
                lowerPath.endsWith(".mp4") ||
                lowerPath.endsWith(".webm") ||
                lowerPath.contains("screenshot") ||
                lowerPath.contains("screen_record") ||
                lowerPath.contains("screencast")

        if (suspicious) {
            violationListener.onViolation(
                "FileMonitor",
                "Suspicious file activity [$eventName] in ${dir.name}/: $path"
            )
        }

        Log.d(TAG, "File event [$eventName] in ${dir.absolutePath}: $path")
    }

    companion object {
        private const val TAG = "FileMonitor"

        private const val WATCH_MASK =
            FileObserver.CREATE or
                    FileObserver.MODIFY or
                    FileObserver.MOVED_TO or
                    FileObserver.CLOSE_WRITE

        private const val ALL_EVENTS_MASK = 0xFFF
    }
}
