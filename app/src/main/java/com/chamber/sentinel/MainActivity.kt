package com.chamber.sentinel

import android.os.Bundle
import android.view.WindowManager
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.chamber.sentinel.ui.EventListFragment

/**
 * Main (and only) activity for Chamber Sentinel.
 *
 * Enforces:
 * - FLAG_SECURE on all windows (prevents screenshots / screen recording)
 * - Fullscreen immersive mode with no system bars
 * - Portrait-only orientation (declared in manifest)
 */
class MainActivity : AppCompatActivity() {

    private lateinit var runtime: ChamberRuntime

    override fun onCreate(savedInstanceState: Bundle?) {
        // FLAG_SECURE must be set before setContentView
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE
        )

        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        // Fullscreen immersive: hide both status and navigation bars
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val controller = WindowInsetsControllerCompat(window, window.decorView)
        controller.hide(WindowInsetsCompat.Type.systemBars())
        controller.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE

        // Initialize the Rust runtime
        runtime = ChamberRuntime.getInstance()
        runtime.initialize()

        // Load the event list fragment
        if (savedInstanceState == null) {
            supportFragmentManager.beginTransaction()
                .replace(R.id.fragment_container, EventListFragment())
                .commit()
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        // Zeroize all Rust state when the activity is fully destroyed
        if (isFinishing) {
            runtime.destroy()
        }
    }
}
