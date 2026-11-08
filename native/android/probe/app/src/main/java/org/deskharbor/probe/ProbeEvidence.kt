package org.deskharbor.probe

import android.content.Context
import android.os.Build
import org.json.JSONObject
import java.io.File

/** Main-thread owned state. Never serializes consent intents, UI text or accessibility events. */
object ProbeEvidence {
    val runId: String = java.util.UUID.randomUUID().toString()
    // Process-owned: immutable outstanding request epochs survive Activity recreation.
    // Process death loses this authority and any late result is rejected.
    lateinit var consentRequests: ConsentRequests; private set
    fun initialize(context: Context) {
        if (::consentRequests.isInitialized) return
        val preferences = context.applicationContext.getSharedPreferences("proof_request_ids", Context.MODE_PRIVATE)
        consentRequests = ConsentRequests(preferences.getInt("next_id", 100)) { next ->
            preferences.edit().putInt("next_id", next).commit()
        }
    }
    var session = SessionState(); private set
    var foreground = false
    var projection = false
    var reader = false
    var display = false
    var accessibility = false
    private var sequence = 0L
    private var latest = "Open capture consent to start an attended session."
    fun transition(state: SessionState) { session = state }
    fun record(context: Context, event: String, details: JSONObject = JSONObject()) {
        val json = JSONObject()
            .put("schema", "deskharbor.android-proof.v1")
            .put("run_id", runId).put("sequence", ++sequence).put("time_unix_ms", System.currentTimeMillis())
            .put("event", event).put("epoch", session.epoch).put("phase", session.phase)
            .put("foreground", foreground).put("projection", projection)
            .put("image_reader", reader).put("virtual_display", display)
            .put("accessibility_connected", accessibility)
            .put("input_authorized", session.permitsInput(accessibility))
            .put("api", Build.VERSION.SDK_INT).put("abi", Build.SUPPORTED_ABIS.joinToString(","))
            .put("device_model", Build.MODEL).put("physical_qualification", "not_run")
            .put("details", details)
        latest = json.toString(2)
        File(context.filesDir, "proof.jsonl").appendText(json.toString() + "\n")
        File(context.filesDir, "state.json").writeText(json.toString(2))
    }
    fun latest() = latest
}
