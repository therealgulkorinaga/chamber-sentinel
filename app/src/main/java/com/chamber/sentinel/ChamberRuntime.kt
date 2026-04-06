package com.chamber.sentinel

import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.atomic.AtomicBoolean

/**
 * High-level Kotlin wrapper around [ChamberBridge].
 *
 * Provides a safe, idiomatic API for the rest of the application.
 * Manages the native runtime pointer and translates JSON responses
 * into Kotlin data classes.
 */
class ChamberRuntime private constructor() {

    private val initialized = AtomicBoolean(false)
    private var nativePtr: Long = 0L

    /**
     * Initialize the Rust runtime. Must be called once before use.
     */
    fun initialize() {
        if (initialized.compareAndSet(false, true)) {
            nativePtr = ChamberBridge.nativeInit()
            Log.i(TAG, "Chamber runtime initialized, ptr=$nativePtr, version=${ChamberBridge.nativeVersion()}")
        }
    }

    /**
     * Destroy the runtime and zeroize all state.
     * After calling this, [initialize] must be called again before use.
     */
    fun destroy() {
        if (initialized.compareAndSet(true, false)) {
            ChamberBridge.nativeDestroy(nativePtr)
            nativePtr = 0L
            Log.i(TAG, "Chamber runtime destroyed")
        }
    }

    /**
     * The version string of the native chamber-core library.
     */
    val version: String
        get() {
            check(initialized.get()) { "Runtime not initialized" }
            return ChamberBridge.nativeVersion()
        }

    /**
     * Create a new world under the camera sentinel grammar.
     * @param objective description of the camera session.
     * @return the world UUID string.
     */
    fun createWorld(
        grammarId: String = DEFAULT_GRAMMAR,
        objective: String = "camera_session"
    ): String {
        check(initialized.get()) { "Runtime not initialized" }
        val json = ChamberBridge.createWorld(nativePtr, grammarId, objective)
        val result = JSONObject(json)
        if (result.has("error")) {
            throw RuntimeException(result.getString("error"))
        }
        return result.getString("world_id")
    }

    /**
     * Create an object in a world.
     * @param worldId UUID of the target world.
     * @param objectType grammar-defined type (e.g., "frame", "detection").
     * @param payloadJson JSON payload string.
     * @return the object UUID string.
     */
    fun createObject(worldId: String, objectType: String, payloadJson: String): String {
        check(initialized.get()) { "Runtime not initialized" }
        val json = ChamberBridge.submitCreateObject(nativePtr, worldId, objectType, payloadJson)
        val result = JSONObject(json)
        if (result.has("error")) {
            throw RuntimeException(result.getString("error"))
        }
        return result.getString("object_id")
    }

    /**
     * Ingest a camera frame into the substrate.
     * @param worldId UUID of the target world.
     * @param frameBytes raw frame data.
     * @param width frame width.
     * @param height frame height.
     * @param timestampMs frame timestamp in milliseconds.
     * @return the object UUID string.
     */
    fun ingestFrame(
        worldId: String,
        frameBytes: ByteArray,
        width: Int,
        height: Int,
        timestampMs: Long
    ): String {
        check(initialized.get()) { "Runtime not initialized" }
        val json = ChamberBridge.ingestFrame(nativePtr, worldId, frameBytes, width, height, timestampMs)
        val result = JSONObject(json)
        if (result.has("error")) {
            throw RuntimeException(result.getString("error"))
        }
        return result.getString("object_id")
    }

    /**
     * Seal an object into a preservable artifact.
     */
    fun sealArtifact(worldId: String, objectId: String): String {
        check(initialized.get()) { "Runtime not initialized" }
        val json = ChamberBridge.submitSealArtifact(nativePtr, worldId, objectId)
        val result = JSONObject(json)
        if (result.has("error")) {
            throw RuntimeException(result.getString("error"))
        }
        return result.getString("artifact_id")
    }

    /**
     * Burn a world and return the burn result.
     * @param mode "auto", "emergency", or "manual".
     */
    fun burn(worldId: String, mode: String = "auto"): BurnResult {
        check(initialized.get()) { "Runtime not initialized" }
        val json = ChamberBridge.burn(nativePtr, worldId, mode)
        return BurnResult.fromJson(json)
    }

    /**
     * Get the semantic residue report for a world.
     */
    fun getResidueReport(worldId: String): ResidueReport {
        check(initialized.get()) { "Runtime not initialized" }
        val json = ChamberBridge.getResidueReport(nativePtr, worldId)
        return ResidueReport.fromJson(json)
    }

    companion object {
        private const val TAG = "ChamberRuntime"
        const val DEFAULT_GRAMMAR = "camera_sentinel_v1"

        @Volatile
        private var instance: ChamberRuntime? = null

        fun getInstance(): ChamberRuntime {
            return instance ?: synchronized(this) {
                instance ?: ChamberRuntime().also { instance = it }
            }
        }
    }
}

/**
 * Result of a burn operation, including the semantic residue report.
 */
data class BurnResult(
    val worldId: String,
    val mode: String,
    val layersCompleted: List<String>,
    val errors: List<String>,
    val residue: ResidueReport?,
) {
    companion object {
        fun fromJson(json: String): BurnResult {
            val obj = JSONObject(json)
            if (obj.has("error")) {
                throw RuntimeException(obj.getString("error"))
            }
            val layers = mutableListOf<String>()
            val layersArr = obj.optJSONArray("layers_completed")
            if (layersArr != null) {
                for (i in 0 until layersArr.length()) {
                    layers.add(layersArr.getString(i))
                }
            }
            val errs = mutableListOf<String>()
            val errsArr = obj.optJSONArray("errors")
            if (errsArr != null) {
                for (i in 0 until errsArr.length()) {
                    errs.add(errsArr.getString(i))
                }
            }
            val residueObj = obj.optJSONObject("residue")
            val residue = residueObj?.let { ResidueReport.fromJsonObject(it) }

            return BurnResult(
                worldId = obj.optString("world_id", ""),
                mode = obj.optString("mode", ""),
                layersCompleted = layers,
                errors = errs,
                residue = residue,
            )
        }
    }
}

/**
 * Semantic residue report from the Rust substrate.
 * Measures what state (if any) survived the burn.
 */
data class ResidueReport(
    val stateEngineHasWorld: Boolean,
    val cryptoKeyExists: Boolean,
    val cryptoKeyDestroyed: Boolean,
    val substrateEventCount: Int,
    val worldEventsSurviving: Int,
    val auditLeaksInternals: Boolean,
    val residueScore: Double,
    val framesProcessed: Long,
    val chambersBurned: Long,
    val eventsSealed: Long,
) {
    companion object {
        fun fromJson(json: String): ResidueReport {
            val obj = JSONObject(json)
            if (obj.has("error")) {
                throw RuntimeException(obj.getString("error"))
            }
            return fromJsonObject(obj)
        }

        fun fromJsonObject(obj: JSONObject): ResidueReport {
            return ResidueReport(
                stateEngineHasWorld = obj.optBoolean("state_engine_has_world", false),
                cryptoKeyExists = obj.optBoolean("crypto_key_exists", false),
                cryptoKeyDestroyed = obj.optBoolean("crypto_key_destroyed", false),
                substrateEventCount = obj.optInt("substrate_event_count", 0),
                worldEventsSurviving = obj.optInt("world_events_surviving", 0),
                auditLeaksInternals = obj.optBoolean("audit_leaks_internals", false),
                residueScore = obj.optDouble("residue_score", 0.0),
                framesProcessed = obj.optLong("frames_processed", 0),
                chambersBurned = obj.optLong("chambers_burned", 0),
                eventsSealed = obj.optLong("events_sealed", 0),
            )
        }
    }
}

/**
 * An audit event from the Rust substrate.
 */
data class AuditEvent(
    val worldId: String,
    val timestamp: String,
    val eventType: String,
    val detail: String,
) {
    companion object {
        fun fromJson(obj: JSONObject): AuditEvent {
            val eventType = obj.optJSONObject("event_type")
            val typeStr = eventType?.keys()?.asSequence()?.firstOrNull() ?: "unknown"
            return AuditEvent(
                worldId = obj.optString("world_id", ""),
                timestamp = obj.optString("timestamp", ""),
                eventType = typeStr,
                detail = eventType?.toString() ?: "",
            )
        }

        fun listFromJson(json: String): List<AuditEvent> {
            val array = JSONArray(json)
            return (0 until array.length()).map { i ->
                fromJson(array.getJSONObject(i))
            }
        }
    }
}
