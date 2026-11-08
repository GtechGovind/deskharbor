package org.deskharbor.probe

class ConsentRequests(initialNextId: Int = 100, private val persistNext: (Int) -> Boolean = { true }) {
    private var nextId = initialNextId
    private val epochs = mutableMapOf<Int, Long>()
    fun issue(epoch: Long): Int {
        check(nextId in 100..65535) { "Consent request IDs exhausted" }
        val reserved = nextId
        check(persistNext(reserved + 1)) { "Cannot persist consent request ID" }
        nextId = reserved + 1
        epochs[reserved] = epoch
        return reserved
    }
    fun take(requestId: Int): Long? = epochs.remove(requestId)
}
