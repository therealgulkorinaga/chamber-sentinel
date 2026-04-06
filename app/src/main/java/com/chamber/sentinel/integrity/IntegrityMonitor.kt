package com.chamber.sentinel.integrity

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import com.chamber.sentinel.R

/**
 * Foreground service that orchestrates all integrity monitors.
 *
 * Starts [NetworkMonitor], [CameraAccessMonitor], [FileMonitor],
 * [ProcessMonitor], and [ScreenCaptureMonitor] on service creation
 * and tears them down on destruction.
 */
class IntegrityMonitor : Service() {

    private lateinit var networkMonitor: NetworkMonitor
    private lateinit var cameraAccessMonitor: CameraAccessMonitor
    private lateinit var fileMonitor: FileMonitor
    private lateinit var processMonitor: ProcessMonitor
    private lateinit var screenCaptureMonitor: ScreenCaptureMonitor

    private val handler = Handler(Looper.getMainLooper())

    /** Listeners can subscribe to integrity violations. */
    fun interface ViolationListener {
        fun onViolation(source: String, description: String)
    }

    private val listeners = mutableListOf<ViolationListener>()

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "IntegrityMonitor service created")

        createNotificationChannel()
        startForeground(NOTIFICATION_ID, buildNotification())

        val violationCallback = ViolationListener { source, description ->
            Log.w(TAG, "VIOLATION [$source]: $description")
            synchronized(listeners) {
                listeners.forEach { it.onViolation(source, description) }
            }
        }

        networkMonitor = NetworkMonitor(this, violationCallback)
        cameraAccessMonitor = CameraAccessMonitor(this, violationCallback)
        fileMonitor = FileMonitor(violationCallback)
        processMonitor = ProcessMonitor(violationCallback)
        screenCaptureMonitor = ScreenCaptureMonitor(this, violationCallback)

        networkMonitor.start()
        cameraAccessMonitor.start()
        fileMonitor.start()
        processMonitor.start(handler)
        screenCaptureMonitor.start()
    }

    override fun onDestroy() {
        Log.i(TAG, "IntegrityMonitor service destroyed")
        networkMonitor.stop()
        cameraAccessMonitor.stop()
        fileMonitor.stop()
        processMonitor.stop()
        screenCaptureMonitor.stop()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        return START_STICKY
    }

    fun addViolationListener(listener: ViolationListener) {
        synchronized(listeners) {
            listeners.add(listener)
        }
    }

    fun removeViolationListener(listener: ViolationListener) {
        synchronized(listeners) {
            listeners.remove(listener)
        }
    }

    // -----------------------------------------------------------------------
    // Notification
    // -----------------------------------------------------------------------

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            getString(R.string.integrity_notification_channel),
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "Chamber Sentinel integrity monitoring"
            setShowBadge(false)
        }

        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(channel)
    }

    private fun buildNotification(): Notification {
        return Notification.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.integrity_notification_title))
            .setContentText(getString(R.string.integrity_notification_text))
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setOngoing(true)
            .build()
    }

    companion object {
        private const val TAG = "IntegrityMonitor"
        private const val CHANNEL_ID = "chamber_sentinel_integrity"
        private const val NOTIFICATION_ID = 1

        fun start(context: Context) {
            val intent = Intent(context, IntegrityMonitor::class.java)
            context.startForegroundService(intent)
        }

        fun stop(context: Context) {
            val intent = Intent(context, IntegrityMonitor::class.java)
            context.stopService(intent)
        }
    }
}
