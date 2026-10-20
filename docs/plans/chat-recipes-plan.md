# Chat recipes

Status: complete
Delivery: [PR #111](https://github.com/robchristie/stobie/pull/111)

## Outcome and boundaries

A chat application shows a conversation's messages, a list of
conversations and on or off settings. Stobie owns four recipes for those: a
message bubble, a conversation row, an unread dot and a switch, each with
semantics that agents and assistive technology can read, at phone and
desktop widths in both themes. The composer's send and attach controls move
to the `Send` and `Attach` icons. Applications own their conversations,
messages, senders, times and unread counts, their settings, and how they
store and send them. Message timestamps and sender names inside a bubble,
reactions, read receipts, typing indicators, swipe actions, grouping of
consecutive messages and an interactive avatar strip are separate work, as
are the toggle and quiet-button gaps S5 left (robchristie/stobie#110 owns
the avatar and icon follow-ups).

## Implementation and UI acceptance contract

Representative task: scan a conversation list for who wrote last and what
is unread, open one conversation and read it back and forth, then turn a
setting off. Names and messages lead; unread state and the current
conversation are noticed second; settings are quiet. Reference: the
composer and avatar recipes, the navigation item's current state, and chat
clients whose own messages sit at the end of the line on an accented fill
and whose conversation lists show an avatar, a name, a preview, a time and
an unread dot.

- Message bubble: `PresentationContext::message_bubble(ui, key,
  MessageBubbleSpec { side, sender }, render)` holds any content the closure
  presents, such as Markdown or reading text. `MessageSide::Own` sits at the
  end of the line on the selection fill, `Other` at the start on the raised
  surface, both with a subtle border and the `chat.bubbleRadius` corners. A
  bubble is at most 85% of its column and `chat.bubbleMaxWidth`, grown with
  text to 150%; it hugs short content by measuring its painted content and
  keeping the width for later passes, measuring again when the text, the way
  it is set or the column changes. Content sees `message_bubble_tokens`
  (surfaces are the fill; muted text that would read below its validated
  contrast becomes primary). Semantics: a `Section` node named by the sender
  holding the content's nodes, and an AccessKit group of that name; the
  content keeps its own text kinds, so its words reach observation as
  content text.
- Conversation row: `Frame::conversation_row(ui, p, BoundAction,
  ConversationRowSpec)` on S5's bound actions and list-row geometry: a
  decorative avatar, a one-line title, a one-line preview, the
  application's formatted time kept whole up to half the title line, the
  unread dot, and the current state. Semantics: a `NavigationItem` named by
  the title, `selected` while current (`aria-current`), whose description is
  the unread words, preview, time and action description; the argument is
  the conversation's ID. The current row keeps the selection fill and
  marker; a hovered row takes the raised surface because the themes fill a
  quiet hover with the selection's colour. Measurements:
  `ConversationRowTitle`, `ConversationRowPreview` and
  `ConversationRowTime` under `ConversationRow`, with no content text.
- Unread dot: `chat.unreadDotSize`, the accent where it stands out at 3:1
  from what is behind it and primary text otherwise; decorative.
- Switch: `Frame::switch(ui, p, target, SwitchSpec { label, on })`, a
  full-width row whose label always shows and whose track and thumb (the
  `switch.trackWidth` by `switch.trackHeight` tokens) show the state, never
  the row's fill, so neither of S5's toggle gaps recurs. Semantics: a new
  `UiRole::Switch`, AccessKit `Switch` with its toggled state, ARIA
  `switch` with `aria-checked`. Measurement: `SwitchLabel`.
- States: unread, current, plain, hovered, pressed, focused and disabled
  rows; long and localised names and previews; a row without a preview; on,
  off and disabled switches; a long switch label; own and other bubbles
  holding text, Markdown, a list, a link, a long link and code; phone,
  regular and desktop widths; 150% text; light, dark and high contrast.
  Excluded: an empty conversation list and loading states
  (application-owned), and scrolling within a bubble (bubbles grow).

Calibration question: does a conversation read as two voices and a list as
who wrote and what is unread, with settings that state themselves, at phone
and desktop widths in both themes? Smallest probe: Gallery
`chat/conversation` in dark comfortable at desktop width and
`chat/long-narrow` in light high contrast at 150%. Evidence owner:
[Chat evidence](../chat-evidence.md). Exit: focused regressions, a browser
journey, inspected baselines and an accepted two-pass critique, with at
most one refinement.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Tokens, roles and text kinds | Complete | Token generation check; protocol and semantic audit regressions |
| Switch, conversation row and unread dot | Complete | 10 recipe regressions, including contrast on every row fill |
| Message bubble and scoped and hosted API | Complete | 6 hosted regressions, including inspection, keyboard and refitting |
| Gallery, browser journey, documentation and baselines | Complete | [Chat evidence](../chat-evidence.md), four inspected baselines, `tools/chat-browser-smoke.sh` |
| Candidate qualification | Complete | `cargo xtask verify` on the candidate; review, CI and merge retained in the PR |

## Closeout

The four recipes, their tokens, the `Switch` role, the scoped and hosted
API, the Gallery stories with four fixtures, the browser journey and the
documentation are complete, and the composer uses the `Send` and `Attach`
icons. Final review, CI and merge qualification remain with the owning pull
request; this completed product state does not assert that it has merged.
