package org.deskharbor.probe

fun releaseAll(vararg releases: () -> Unit): Int {
    var failures = 0
    releases.forEach { try { it() } catch (_: Exception) { failures++ } }
    return failures
}
fun <T : AutoCloseable> configureOrClose(resource: T, configure: (T) -> Unit): T {
    try { configure(resource); return resource }
    catch (failure: Exception) {
        try { resource.close() } catch (closeFailure: Exception) { failure.addSuppressed(closeFailure) }
        throw failure
    }
}
