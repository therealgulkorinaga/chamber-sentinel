package com.chamber.sentinel

/**
 * JNI bridge to the Rust chamber-core cdylib.
 *
 * All methods are static native calls into libchamber_core.so.
 * The Rust side uses a pointer-based API: nativeInit returns a handle,
 * and all subsequent calls pass it as the first argument.
 */
object ChamberBridge {

    init {
        System.loadLibrary("chamber_core")
    }

    /**
     * Initialize the Rust runtime. Returns a pointer handle to the Runtime.
     * Must be called before any other native method.
     */
    @JvmStatic
    external fun nativeInit(): Long

    /**
     * Tear down the Rust runtime and zeroize all in-memory state.
     * @param ptr the runtime handle returned by [nativeInit].
     */
    @JvmStatic
    external fun nativeDestroy(ptr: Long)

    /**
     * Return the version string of the Rust chamber-core crate.
     */
    @JvmStatic
    external fun nativeVersion(): String

    /**
     * Create a new Chamber world under a grammar.
     * Returns a JSON string with the world UUID: {"world_id": "..."}.
     * @param ptr runtime handle.
     * @param grammarId the grammar identifier (e.g., "camera_sentinel_v1").
     * @param objective description of the session objective.
     */
    @JvmStatic
    external fun createWorld(ptr: Long, grammarId: String, objective: String): String

    /**
     * Create a new object in a world.
     * @param ptr runtime handle.
     * @param worldId UUID string of the target world.
     * @param objectType the grammar-defined object type.
     * @param payloadJson JSON string of the object payload.
     * @return JSON string with object_id.
     */
    @JvmStatic
    external fun submitCreateObject(
        ptr: Long,
        worldId: String,
        objectType: String,
        payloadJson: String
    ): String

    /**
     * Seal an object into a preservable artifact.
     * @param ptr runtime handle.
     * @param worldId UUID string of the world.
     * @param objectId UUID string of the object to seal.
     * @return JSON string with artifact_id.
     */
    @JvmStatic
    external fun submitSealArtifact(ptr: Long, worldId: String, objectId: String): String

    /**
     * Burn a world using the six-layer destruction protocol.
     * @param ptr runtime handle.
     * @param worldId UUID string of the world to burn.
     * @param mode termination mode: "auto", "emergency", or "manual".
     * @return JSON string of the BurnResult including semantic residue report.
     */
    @JvmStatic
    external fun burn(ptr: Long, worldId: String, mode: String): String

    /**
     * Get the semantic residue report for a world (post-burn measurement).
     * @param ptr runtime handle.
     * @param worldId UUID string of the world.
     * @return JSON string of the SemanticResidueReport.
     */
    @JvmStatic
    external fun getResidueReport(ptr: Long, worldId: String): String

    /**
     * Ingest a camera frame into the substrate.
     * @param ptr runtime handle.
     * @param worldId UUID string of the world.
     * @param frameBytes raw frame data.
     * @param width frame width in pixels.
     * @param height frame height in pixels.
     * @param timestamp frame timestamp in milliseconds.
     * @return JSON string with object_id and frame_size.
     */
    @JvmStatic
    external fun ingestFrame(
        ptr: Long,
        worldId: String,
        frameBytes: ByteArray,
        width: Int,
        height: Int,
        timestamp: Long
    ): String
}
