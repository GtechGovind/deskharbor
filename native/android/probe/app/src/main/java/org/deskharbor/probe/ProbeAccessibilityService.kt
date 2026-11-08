package org.deskharbor.probe

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.app.Activity
import android.content.Intent
import android.graphics.Path
import android.graphics.Rect
import android.view.View
import android.view.Display
import android.view.accessibility.AccessibilityEvent

class ProbeAccessibilityService : AccessibilityService() {
    companion object { var instance: ProbeAccessibilityService? = null; private set }
    override fun onServiceConnected() { instance = this; ProbeEvidence.accessibility = true; ProbeEvidence.record(this, "accessibility_connected_by_os") }
    override fun onAccessibilityEvent(event: AccessibilityEvent?) { /* Deliberately do not inspect or retain event contents. */ }
    override fun onInterrupt() { ProbeEvidence.record(this, "accessibility_interrupted") }
    override fun onUnbind(intent: Intent?): Boolean { instance = null; ProbeEvidence.accessibility = false; ProbeEvidence.record(this, "accessibility_unbound"); return super.onUnbind(intent) }
    override fun onDestroy() { instance = null; ProbeEvidence.accessibility = false; ProbeEvidence.record(this, "accessibility_disconnected"); super.onDestroy() }
    fun tapLocalTarget(activity: Activity, target: View) {
        val visible = Rect()
        val localX = target.width / 2; val localY = target.height / 2
        if (!activity.hasWindowFocus() || activity.isInMultiWindowMode || target.display?.displayId != Display.DEFAULT_DISPLAY ||
            !target.isShown || !target.getLocalVisibleRect(visible) || !visible.contains(localX, localY) ||
            !ProbeEvidence.session.permitsInput(ProbeEvidence.accessibility)) { ProbeEvidence.record(this, "input_denied"); return }
        val screen = IntArray(2); target.getLocationOnScreen(screen)
        val x = screen[0] + localX.toFloat(); val y = screen[1] + localY.toFloat()
        val epoch = ProbeEvidence.session.epoch
        val gesture = GestureDescription.Builder().setDisplayId(Display.DEFAULT_DISPLAY)
            .addStroke(GestureDescription.StrokeDescription(Path().apply { moveTo(x, y) }, 0, 80)).build()
        val accepted = dispatchGesture(gesture, object : GestureResultCallback() {
            override fun onCompleted(gestureDescription: GestureDescription?) { ProbeEvidence.record(this@ProbeAccessibilityService, "input_gesture_completed", org.json.JSONObject().put("requested_epoch", epoch)) }
            override fun onCancelled(gestureDescription: GestureDescription?) { ProbeEvidence.record(this@ProbeAccessibilityService, "input_gesture_cancelled", org.json.JSONObject().put("requested_epoch", epoch)) }
        }, null)
        ProbeEvidence.record(this, if (accepted) "input_gesture_dispatched" else "input_gesture_rejected")
    }
}
