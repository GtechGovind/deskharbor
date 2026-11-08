package org.deskharbor.probe

import org.junit.Assert.*
import org.junit.Test

class ResourceCleanupTest {
    @Test fun oneReleaseFailureCannotKeepOtherResourcesAlive() {
        var readerClosed = false
        var foregroundStopped = false
        val errors = releaseAll({ throw IllegalStateException("display release failed") }, { readerClosed = true }, { foregroundStopped = true })
        assertTrue(readerClosed)
        assertTrue(foregroundStopped)
        assertEquals(1, errors)
    }
    @Test fun failedSurfaceConfigurationClosesReplacement() {
        class Resource : AutoCloseable { var closed = false; override fun close() { closed = true } }
        val replacement = Resource()
        try { configureOrClose(replacement) { throw IllegalStateException("surface assignment failed") }; fail("Must propagate configuration failure") }
        catch (_: IllegalStateException) { }
        assertTrue(replacement.closed)
    }
}
