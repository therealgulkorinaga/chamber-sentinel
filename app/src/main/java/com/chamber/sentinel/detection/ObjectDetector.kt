package com.chamber.sentinel.detection

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageFormat
import android.graphics.Rect
import android.graphics.YuvImage
import android.util.Log
import org.tensorflow.lite.Interpreter
import org.tensorflow.lite.support.common.FileUtil
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * On-device object detection using EfficientDet-Lite0 (INT8 quantized).
 *
 * Input: raw camera frame bytes
 * Output: list of Detection (class label + confidence)
 * No bounding boxes stored — only event labels survive.
 */
class ObjectDetector(context: Context) {

    data class Detection(
        val label: String,
        val confidence: Float
    )

    private val interpreter: Interpreter
    private val inputSize = 300 // SSD MobileNet V1 expects 300x300
    private val labels = listOf(
        "person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck",
        "boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench",
        "bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra",
        "giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee",
        "skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove",
        "skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup",
        "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange",
        "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "couch",
        "potted plant", "bed", "dining table", "toilet", "tv", "laptop", "mouse",
        "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
        "refrigerator", "book", "clock", "vase", "scissors", "teddy bear",
        "hair drier", "toothbrush"
    )

    // Map COCO labels to our 6 event types
    private val eventTypeMap = mapOf(
        "person" to "person_detected",
        "bicycle" to "vehicle_detected", "car" to "vehicle_detected",
        "motorcycle" to "vehicle_detected", "bus" to "vehicle_detected",
        "truck" to "vehicle_detected", "train" to "vehicle_detected",
        "bird" to "animal_detected", "cat" to "animal_detected",
        "dog" to "animal_detected", "horse" to "animal_detected",
        "sheep" to "animal_detected", "cow" to "animal_detected",
        "bear" to "animal_detected",
        "backpack" to "package_detected", "suitcase" to "package_detected",
        "handbag" to "package_detected",
    )

    companion object {
        private const val TAG = "ObjectDetector"
        private const val CONFIDENCE_THRESHOLD = 0.10f
        private const val MODEL_FILE = "detect_model.tflite"
    }

    init {
        val model = FileUtil.loadMappedFile(context, MODEL_FILE)
        val options = Interpreter.Options().apply {
            setNumThreads(2)
        }
        interpreter = Interpreter(model, options)
        Log.i(TAG, "Model loaded: $MODEL_FILE, input size: ${inputSize}x${inputSize}")
    }

    /**
     * Run detection on JPEG frame bytes.
     * Returns list of detections above confidence threshold.
     */
    fun detect(jpegBytes: ByteArray): List<Detection> {
        return try {
            Log.d(TAG, "Detecting on ${jpegBytes.size} bytes")
            val bitmap = BitmapFactory.decodeByteArray(jpegBytes, 0, jpegBytes.size)
            if (bitmap == null) {
                Log.w(TAG, "BitmapFactory returned null — bytes may not be valid JPEG")
                return emptyList()
            }
            Log.d(TAG, "Bitmap decoded: ${bitmap.width}x${bitmap.height}")
            val results = detectFromBitmap(bitmap)
            bitmap.recycle()
            Log.d(TAG, "Detections: ${results.size} — ${results.map { "${it.label}(${it.confidence})" }}")
            results
        } catch (e: Exception) {
            Log.e(TAG, "Detection failed: ${e.message}")
            emptyList()
        }
    }

    /**
     * Run detection on a Bitmap.
     */
    private fun detectFromBitmap(bitmap: Bitmap): List<Detection> {
        // Resize to model input size
        val resized = Bitmap.createScaledBitmap(bitmap, inputSize, inputSize, true)

        // Prepare input tensor (UINT8 quantized)
        val inputBuffer = ByteBuffer.allocateDirect(1 * inputSize * inputSize * 3)
        inputBuffer.order(ByteOrder.nativeOrder())
        inputBuffer.rewind()

        val pixels = IntArray(inputSize * inputSize)
        resized.getPixels(pixels, 0, inputSize, 0, 0, inputSize, inputSize)
        for (pixel in pixels) {
            inputBuffer.put(((pixel shr 16) and 0xFF).toByte()) // R
            inputBuffer.put(((pixel shr 8) and 0xFF).toByte())  // G
            inputBuffer.put((pixel and 0xFF).toByte())           // B
        }
        resized.recycle()

        // SSD MobileNet V1 outputs: [1,10,4] boxes, [1,10] classes, [1,10] scores, [1] count
        val boxes = Array(1) { Array(10) { FloatArray(4) } }
        val classes = Array(1) { FloatArray(10) }
        val scores = Array(1) { FloatArray(10) }
        val count = FloatArray(1)

        val outputs = mapOf(
            0 to boxes,
            1 to classes,
            2 to scores,
            3 to count
        )

        // Run inference
        interpreter.runForMultipleInputsOutputs(arrayOf(inputBuffer), outputs)

        // Log raw outputs for debugging
        val numDetections = count[0].toInt().coerceAtMost(10)
        Log.d(TAG, "Raw: count=${count[0]}, scores=${scores[0].take(5).map { "%.3f".format(it) }}, classes=${classes[0].take(5).map { it.toInt() }}")

        // Extract detections above threshold
        val results = mutableListOf<Detection>()

        for (i in 0 until numDetections) {
            val score = scores[0][i]
            if (score >= CONFIDENCE_THRESHOLD) {
                val classIndex = classes[0][i].toInt()
                val label = if (classIndex in labels.indices) labels[classIndex] else "unknown"
                val eventType = eventTypeMap[label] ?: "unknown_object"
                results.add(Detection(label = eventType, confidence = score))
            }
        }

        // Deduplicate: keep highest confidence per event type
        return results
            .groupBy { it.label }
            .map { (label, detections) ->
                Detection(label, detections.maxOf { it.confidence })
            }
    }

    fun close() {
        interpreter.close()
    }
}
