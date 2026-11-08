package org.deskharbor.probe

import org.junit.Assert.*
import org.junit.Test

class SessionStateTest {
    @Test fun failedRequestIdPersistenceNeverIssuesAuthority() {
        val requests = ConsentRequests(100) { false }
        try { requests.issue(8); fail("Must fail closed when request ID cannot be persisted") }
        catch (_: IllegalStateException) { }
        assertNull(requests.take(100))
    }
    @Test fun oldProcessResultCannotConsumeNewProcessRequest() {
        var persistedNext = 100
        val aProcess = ConsentRequests(persistedNext) { persistedNext = it; true }
        val aId = aProcess.issue(1)
        val bProcess = ConsentRequests(persistedNext) { persistedNext = it; true }
        val bId = bProcess.issue(1)
        assertNotEquals(aId, bId)
        assertNull(bProcess.take(aId))
        assertEquals(1L, bProcess.take(bId))
    }
    @Test fun failedVirtualDisplayCreationNeverAuthorizesInput() {
        val pending = SessionState().request().consent(true)
        val failed = pending.started(pending.epoch, false)
        assertEquals("starting", failed.phase)
        assertFalse(failed.permitsInput(true))
        assertFalse(failed.acceptsFrame(pending.epoch))
    }
    // Catches a delayed A grant or denial consuming new B's consent prompt.
    @Test fun delayedConsentCannotAuthorizeOrDenyNewEpoch() {
        val a = SessionState().request()
        val b = a.stop().request()
        assertEquals(b, b.consent(a.epoch, true))
        assertEquals(b, b.consent(a.epoch, false))
        val granted = b.consent(b.epoch, true)
        assertEquals("starting", granted.phase)
        assertEquals(granted, granted.consent(b.epoch, true))
    }
    // Catches replacing a request's immutable origin epoch or reusing a consumed ID.
    @Test fun requestIdsResolveOnceToTheirOriginalEpoch() {
        val requests = ConsentRequests()
        val a = requests.issue(17)
        val b = requests.issue(18)
        assertNotEquals(a, b)
        assertEquals(17L, requests.take(a))
        assertNull(requests.take(a))
        assertEquals(18L, requests.take(b))
        assertNull(requests.take(999))
    }
    // Catches accepting frames/input before both consent and an active foreground projection.
    @Test fun consentAloneDoesNotAuthorizeFramesOrInput() {
        val awaiting = SessionState().request()
        assertEquals("awaiting_consent", awaiting.phase)
        assertFalse(awaiting.acceptsFrame(awaiting.epoch))
        val granted = awaiting.consent(true)
        assertEquals("starting", granted.phase)
        assertFalse(granted.permitsInput(true))
        val capturing = granted.started(granted.epoch, true)
        assertTrue(capturing.acceptsFrame(granted.epoch))
        assertTrue(capturing.permitsInput(true))
        assertFalse(capturing.permitsInput(false))
    }
    // Catches granting a denied token or accepting an asynchronous frame after cleanup.
    @Test fun denialAndStopRevokeAccess() {
        val denied = SessionState().request().consent(false)
        assertEquals("denied", denied.phase)
        assertFalse(denied.acceptsFrame(denied.epoch))
        val active = SessionState().request().consent(true).let { it.started(it.epoch, true) }
        val stopped = active.stop()
        assertEquals("stopped", stopped.phase)
        assertFalse(stopped.acceptsFrame(active.epoch))
        assertFalse(stopped.permitsInput(true))
        assertEquals(stopped, stopped.stop())
    }
    // Catches old callbacks acquiring authority during a subsequent consent session.
    @Test fun oldEpochCannotStartOrCaptureANewSession() {
        val old = SessionState().request().consent(true)
        val next = old.stop().request().consent(true)
        assertEquals(2L, next.epoch)
        assertEquals("starting", next.started(old.epoch, true).phase)
        assertFalse(next.started(next.epoch, true).acceptsFrame(old.epoch))
        assertEquals("capturing", next.started(next.epoch, true).phase)
    }
    // Catches duplicate requests consuming an active session and consent outside a prompt.
    @Test fun unsolicitedConsentAndRepeatedRequestsHaveNoAuthority() {
        assertEquals("idle", SessionState().consent(true).phase)
        val awaiting = SessionState().request()
        assertEquals(awaiting, awaiting.request())
        val active = awaiting.consent(true).let { it.started(it.epoch, true) }
        assertEquals(active, active.request())
        assertEquals("stopped", active.stop().started(active.epoch, true).phase)
    }
}
