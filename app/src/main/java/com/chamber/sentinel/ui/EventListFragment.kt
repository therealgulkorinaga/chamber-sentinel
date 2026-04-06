package com.chamber.sentinel.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.fragment.app.Fragment
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.chamber.sentinel.AuditEvent
import com.chamber.sentinel.R

/**
 * Fragment displaying the live list of audit events from the Rust substrate.
 *
 * Events are displayed in reverse chronological order in a RecyclerView.
 */
class EventListFragment : Fragment() {

    private lateinit var recyclerView: RecyclerView
    private lateinit var emptyView: TextView
    private val adapter = EventAdapter()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        val root = RecyclerView(requireContext()).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(0xFF0A0A0A.toInt())
        }

        // Build layout programmatically to avoid extra XML files
        val frame = android.widget.FrameLayout(requireContext()).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }

        recyclerView = RecyclerView(requireContext()).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            layoutManager = LinearLayoutManager(context).apply {
                reverseLayout = true
                stackFromEnd = true
            }
            adapter = this@EventListFragment.adapter
            setBackgroundColor(0xFF0A0A0A.toInt())
        }

        emptyView = TextView(requireContext()).apply {
            layoutParams = android.widget.FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = android.view.Gravity.CENTER
            }
            text = getString(R.string.event_list_empty)
            setTextColor(0xFF666666.toInt())
            textSize = 16f
        }

        frame.addView(recyclerView)
        frame.addView(emptyView)

        updateEmptyState()
        return frame
    }

    /**
     * Add an event to the list and scroll to it.
     */
    fun addEvent(event: AuditEvent) {
        adapter.addEvent(event)
        updateEmptyState()
        recyclerView.scrollToPosition(adapter.itemCount - 1)
    }

    /**
     * Replace all events.
     */
    fun setEvents(events: List<AuditEvent>) {
        adapter.setEvents(events)
        updateEmptyState()
    }

    private fun updateEmptyState() {
        if (::emptyView.isInitialized && ::recyclerView.isInitialized) {
            val empty = adapter.itemCount == 0
            emptyView.visibility = if (empty) View.VISIBLE else View.GONE
            recyclerView.visibility = if (empty) View.GONE else View.VISIBLE
        }
    }

    // -----------------------------------------------------------------------
    // RecyclerView Adapter
    // -----------------------------------------------------------------------

    private class EventAdapter : RecyclerView.Adapter<EventViewHolder>() {

        private val events = mutableListOf<AuditEvent>()

        fun addEvent(event: AuditEvent) {
            events.add(event)
            notifyItemInserted(events.size - 1)
        }

        fun setEvents(newEvents: List<AuditEvent>) {
            events.clear()
            events.addAll(newEvents)
            notifyDataSetChanged()
        }

        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): EventViewHolder {
            val view = LayoutInflater.from(parent.context)
                .inflate(android.R.layout.simple_list_item_2, parent, false)
            return EventViewHolder(view)
        }

        override fun onBindViewHolder(holder: EventViewHolder, position: Int) {
            holder.bind(events[position])
        }

        override fun getItemCount(): Int = events.size
    }

    private class EventViewHolder(view: View) : RecyclerView.ViewHolder(view) {

        private val text1: TextView = view.findViewById(android.R.id.text1)
        private val text2: TextView = view.findViewById(android.R.id.text2)

        fun bind(event: AuditEvent) {
            text1.text = "[${event.eventType}] ${event.detail}"
            text1.setTextColor(colorForEventType(event.eventType))
            text1.textSize = 14f

            text2.text = "${event.timestamp} | ${event.worldId}"
            text2.setTextColor(0xFF888888.toInt())
            text2.textSize = 11f

            itemView.setBackgroundColor(0xFF0A0A0A.toInt())
            itemView.setPadding(24, 16, 24, 16)
        }

        private fun colorForEventType(eventType: String): Int = when {
            eventType.contains("Violation") || eventType.contains("violation") ->
                0xFFFF4444.toInt()
            eventType.contains("Burn") || eventType.contains("burn") ->
                0xFFFF8800.toInt()
            eventType.contains("WorldCreated") || eventType.contains("Created") ->
                0xFF00FF41.toInt()
            else -> 0xFFCCCCCC.toInt()
        }
    }
}
