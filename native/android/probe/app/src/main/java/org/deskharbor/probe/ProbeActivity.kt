package org.deskharbor.probe

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

class ProbeActivity : Activity() {
    private lateinit var status: TextView
    private lateinit var target: Button
    private val handler = Handler(Looper.getMainLooper())
    private var visible = false
    private var hits = 0
    private val refresh = object : Runnable {
        override fun run() { status.text = ProbeEvidence.latest(); handler.postDelayed(this, 500) }
    }
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        ProbeEvidence.initialize(this)
        val column = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(24, 32, 24, 24) }
        fun button(label: String, action: () -> Unit): Button = Button(this).also {
            it.text = label; it.minimumHeight = 96; it.setOnClickListener { action() }; column.addView(it)
        }
        column.addView(TextView(this).apply { text = "DeskHarbor P0 — attended Android probe\nSaves one local PNG per session and JSON evidence in private app storage. Every start opens Android consent. Input is optional and limited to the visible test target. No network or boot hosting."; textSize = 16f })
        button("Start capture — ask Android consent") {
            val next = ProbeEvidence.session.request()
            if (next.phase != "awaiting_consent" || next == ProbeEvidence.session) return@button
            val requestId = try { ProbeEvidence.consentRequests.issue(next.epoch) }
                catch (_: IllegalStateException) { ProbeEvidence.record(this, "consent_request_unavailable"); return@button }
            ProbeEvidence.transition(next); ProbeEvidence.record(this, "consent_requested")
            startActivityForResult(getSystemService(MediaProjectionManager::class.java).createScreenCaptureIntent(), requestId)
        }
        button("Stop capture and release resources") { startService(Intent(this, ProjectionService::class.java).setAction(ProjectionService.STOP)) }
        button("Open Android Accessibility Settings") { startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)) }
        button("Dispatch one tap to local target") {
            if (!visible || !ProbeEvidence.session.permitsInput(ProbeEvidence.accessibility)) {
                ProbeEvidence.record(this, "input_denied"); return@button
            }
            ProbeAccessibilityService.instance?.tapLocalTarget(this, target)
        }
        target = button("Local input target — hits: 0") { hits++; target.text = "Local input target — hits: $hits"; ProbeEvidence.record(this, "local_target_activated") }
        status = TextView(this).apply { textSize = 11f; setTextIsSelectable(true) }; column.addView(status)
        setContentView(ScrollView(this).apply { addView(column) })
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 42)
        ProbeEvidence.record(this, "activity_created")
    }
    override fun onResume() { super.onResume(); visible = true; handler.post(refresh) }
    override fun onPause() { visible = false; handler.removeCallbacks(refresh); super.onPause() }
    @Deprecated("Native probe intentionally uses framework Activity results without AndroidX")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val requestEpoch = ProbeEvidence.consentRequests.take(requestCode) ?: return
        val granted = resultCode == RESULT_OK && data != null
        val previous = ProbeEvidence.session
        val next = previous.consent(requestEpoch, granted)
        if (next == previous) {
            ProbeEvidence.record(this, "stale_consent_ignored", org.json.JSONObject().put("request_epoch", requestEpoch))
            return
        }
        ProbeEvidence.transition(next)
        ProbeEvidence.record(this, if (granted) "consent_granted" else "consent_denied")
        if (granted && ProbeEvidence.session.phase == "starting") startForegroundService(
            Intent(this, ProjectionService::class.java).putExtra("result_code", resultCode).putExtra("consent_data", data).putExtra("epoch", requestEpoch))
    }
}
