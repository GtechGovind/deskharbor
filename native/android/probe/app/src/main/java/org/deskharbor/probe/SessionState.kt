package org.deskharbor.probe

data class SessionState(val epoch: Long = 0, val phase: String = "idle") {
    fun request() = if (phase in setOf("idle", "denied", "stopped")) copy(epoch = epoch + 1, phase = "awaiting_consent") else this
    fun consent(granted: Boolean) = if (phase == "awaiting_consent") copy(phase = if (granted) "starting" else "denied") else this
    fun consent(token: Long, granted: Boolean) = if (token == epoch) consent(granted) else this
    fun started(token: Long, resourcesReady: Boolean) = if (resourcesReady && phase == "starting" && token == epoch) copy(phase = "capturing") else this
    fun stop() = copy(phase = "stopped")
    fun acceptsFrame(token: Long): Boolean = phase == "capturing" && token == epoch
    fun permitsInput(accessibilityEnabled: Boolean): Boolean = phase == "capturing" && accessibilityEnabled
}
