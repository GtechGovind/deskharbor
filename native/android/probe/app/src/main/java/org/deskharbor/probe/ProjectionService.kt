package org.deskharbor.probe

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.ImageReader
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.*
import android.view.WindowManager
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest
import kotlin.math.max

class ProjectionService : Service() {
    companion object { const val STOP = "org.deskharbor.probe.STOP" }
    private var projection: MediaProjection? = null
    private var reader: ImageReader? = null
    private var display: VirtualDisplay? = null
    private var epoch = 0L
    private var stopping = false
    private var saved = false
    private val main = Handler(Looper.getMainLooper())
    private val callback = object : MediaProjection.Callback() {
        override fun onStop() { finish("projection_callback_stop", false) }
        override fun onCapturedContentResize(width: Int, height: Int) {
            if (display != null && width > 0 && height > 0) resize(width, height)
        }
    }
    override fun onBind(intent: Intent?) = null
    @Suppress("DEPRECATION")
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (stopping) {
            ProbeEvidence.transition(ProbeEvidence.session.stop())
            try { ProbeEvidence.record(this, "start_rejected_while_stopping") } finally { stopSelf(startId) }
            return START_NOT_STICKY
        }
        if (intent?.action == STOP) { finish("explicit_stop", true); return START_NOT_STICKY }
        val token = intent?.getLongExtra("epoch", -1) ?: -1
        val consent = intent?.getParcelableExtra<Intent>("consent_data")
        if (projection != null || ProbeEvidence.session.phase != "starting" || token != ProbeEvidence.session.epoch || consent == null || intent.getIntExtra("result_code", 0) != Activity.RESULT_OK) {
            if (projection == null) finish("invalid_start_rejected", true)
            return START_NOT_STICKY
        }
        epoch = token
        try {
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(NotificationChannel("capture", "Attended P0 screen capture", NotificationManager.IMPORTANCE_LOW))
            val stop = PendingIntent.getService(this, 1, Intent(this, ProjectionService::class.java).setAction(STOP), PendingIntent.FLAG_IMMUTABLE)
            val open = PendingIntent.getActivity(this, 2, Intent(this, ProbeActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
            val notification = Notification.Builder(this, "capture").setSmallIcon(android.R.drawable.ic_menu_camera)
                .setContentTitle("DeskHarbor P0 capture is active").setContentText("Attended local probe. Tap Stop to release capture.")
                .setContentIntent(open).setOngoing(true).addAction(Notification.Action.Builder(null, "Stop", stop).build()).build()
            startForeground(1, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
            ProbeEvidence.foreground = true
            ProbeEvidence.record(this, "foreground_started")
            projection = getSystemService(MediaProjectionManager::class.java).getMediaProjection(Activity.RESULT_OK, consent)
            ProbeEvidence.projection = projection != null
            projection!!.registerCallback(callback, main)
            val bounds = getSystemService(WindowManager::class.java).maximumWindowMetrics.bounds
            val (width, height) = boundedSize(bounds.width(), bounds.height())
            reader = newReader(width, height)
            ProbeEvidence.reader = true
            display = checkNotNull(projection!!.createVirtualDisplay("DeskHarborP0", width, height, resources.displayMetrics.densityDpi,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, reader!!.surface, null, main))
            ProbeEvidence.display = true
            ProbeEvidence.transition(ProbeEvidence.session.started(epoch, projection != null && reader != null && display != null))
            ProbeEvidence.record(this, "projection_started", JSONObject().put("width", width).put("height", height))
        } catch (failure: Exception) {
            // Exception class only: messages may contain device or consent-specific data.
            ProbeEvidence.record(this, "projection_error", JSONObject().put("exception_class", failure.javaClass.simpleName))
            finish("start_failed", true)
        }
        return START_NOT_STICKY
    }
    private fun boundedSize(width: Int, height: Int): Pair<Int, Int> {
        val scale = minOf(1.0, 1280.0 / max(width, height))
        return max(1, (width * scale).toInt()) to max(1, (height * scale).toInt())
    }
    private fun newReader(width: Int, height: Int): ImageReader = configureOrClose(ImageReader.newInstance(width, height, PixelFormat.RGBA_8888, 2)) {
        it.setOnImageAvailableListener({ source -> capture(source) }, main)
    }
    private fun resize(rawWidth: Int, rawHeight: Int) {
        val (width, height) = boundedSize(rawWidth, rawHeight)
        if (reader?.width == width && reader?.height == height) return
        try {
            val old = reader
            val replacement = configureOrClose(newReader(width, height)) {
                checkNotNull(display).resize(width, height, resources.displayMetrics.densityDpi)
                checkNotNull(display).surface = it.surface
            }
            reader = replacement
            old?.setOnImageAvailableListener(null, null); old?.close()
            ProbeEvidence.record(this, "capture_resized", JSONObject().put("width", width).put("height", height))
        } catch (failure: Exception) { finish("resize_failed", true) }
    }
    private fun capture(source: ImageReader) {
        if (stopping || source !== reader) return
        val image = try { source.acquireLatestImage() } catch (_: IllegalStateException) { null } ?: return
        try {
            if (saved || !ProbeEvidence.session.acceptsFrame(epoch)) return
            val plane = image.planes[0]
            check(plane.pixelStride == 4 && plane.rowStride >= image.width * 4)
            val bitmap = Bitmap.createBitmap(image.width, image.height, Bitmap.Config.ARGB_8888)
            try {
                val pixels = IntArray(image.width * image.height)
                val buffer = plane.buffer
                for (y in 0 until image.height) for (x in 0 until image.width) {
                    val i = y * plane.rowStride + x * 4
                    val r = buffer.get(i).toInt() and 255; val g = buffer.get(i + 1).toInt() and 255
                    val b = buffer.get(i + 2).toInt() and 255; val a = buffer.get(i + 3).toInt() and 255
                    pixels[y * image.width + x] = (a shl 24) or (r shl 16) or (g shl 8) or b
                }
                bitmap.setPixels(pixels, 0, image.width, 0, 0, image.width, image.height)
                pixels.fill(0)
                val file = File(filesDir, "capture-${ProbeEvidence.runId}-$epoch.png")
                file.outputStream().use { check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
                val digest = MessageDigest.getInstance("SHA-256").digest(file.readBytes()).joinToString("") { "%02x".format(it) }
                saved = true
                ProbeEvidence.record(this, "frame_saved", JSONObject().put("file", file.name).put("sha256", digest)
                    .put("width", image.width).put("height", image.height).put("bytes", file.length()))
            } finally { bitmap.recycle() }
        } catch (failure: Exception) {
            ProbeEvidence.record(this, "frame_error", JSONObject().put("exception_class", failure.javaClass.simpleName))
            finish("capture_failed", true)
        } finally { image.close() }
    }
    private fun finish(reason: String, stopProjection: Boolean) {
        if (stopping) return
        stopping = true
        ProbeEvidence.transition(ProbeEvidence.session.stop()) // Revoke input/frame authority before releasing OS resources.
        val previousProjection = projection; projection = null
        val previousDisplay = display; display = null
        val previousReader = reader; reader = null
        val cleanupErrors = releaseAll(
            { previousDisplay?.release() },
            { previousReader?.setOnImageAvailableListener(null, null) },
            { previousReader?.close() },
            { previousProjection?.unregisterCallback(callback) },
            { if (stopProjection) previousProjection?.stop() },
            { stopForeground(STOP_FOREGROUND_REMOVE) }
        )
        ProbeEvidence.display = false
        ProbeEvidence.reader = false
        ProbeEvidence.projection = false
        ProbeEvidence.foreground = false
        try { ProbeEvidence.record(this, reason, JSONObject().put("cleanup_errors", cleanupErrors)) }
        finally { stopSelf() }
    }
    override fun onTaskRemoved(rootIntent: Intent?) { finish("task_removed", true); super.onTaskRemoved(rootIntent) }
    override fun onDestroy() { if (!stopping) finish("service_destroyed", true); super.onDestroy() }
}
