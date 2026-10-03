---
name: bdfl-chat
description: The mothergod BDFL's chat-wake procedure, for a wake where the operator is at a phone waiting, meaning TRIGGER_EVENT is workflow_dispatch or `.github/scripts/inbox drain` prints pending messages. Read the chat log, drain, act on each message as operator steering, answer each with one Telegram reply carrying the run's status line in the last one, then run the delta core as budget allows. Not for a dispatch whose inbox drains empty, which is a DELTA wake, and not for the scheduled clock tick.
user-invocable: true
---

# BDFL chat wake

The operator typed something and is waiting. Reply latency outranks
every standing duty, a due deep survey included: the next scheduled
wake catches up on whatever the chat crowds out.

## Procedure

1. `.github/scripts/inbox chatlog -n 40` before anything else. Runs
   are amnesiac; the log, written by the worker and tg-send only and
   never by hand, is the conversation so far. An entry carrying this
   run's id means you already spoke (#183).
2. `.github/scripts/inbox drain` prints every pending message with its
   key, oldest first. `inbox ack <key>` on each as you take it up: the
   reaction tells the operator a run has it, in place of the filler
   sentence a person would not send.
3. Do what each message asks, as operator steering. Match the
   operator's register; never re-explain what the thread already shows.
4. Then the delta core from the prompt, cutting whatever the chat work
   crowded out.
5. Answer once the run is otherwise done: one `tg-send --reply-to
   <message_id>` per message, the status line folded into the last
   reply, so the wake never posts twice. `inbox done <key>` after each
   reply lands. The reply is the receipt, so a message stays queued
   only if the run crashed before replying.
6. Before the last send, `inbox drain` again and answer whatever
   arrived mid-run.

`inbox`'s docstring owns the verbs and the UNREADABLE contract;
`tg-send`'s owns escaping, linking and the chat-log write.
