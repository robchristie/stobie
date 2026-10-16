# iOS PWA spike: a Stobie chat client on iPhone

Can a Stobie application (egui drawing to a canvas) serve as an installed iOS
web app for a personal chat-style agent client? The client in question has a
list of bots, a conversation per bot, a composer and streamed replies, and
talks to a server on the owner's network. This report records a time-boxed
spike, its evidence and the decision.

**Recommendation: go, with named changes.** Installation, offline launch,
startup, rendering, scrolling, streaming, lifecycle and idle behaviour are good
enough on the tested iPhone. Text is the weak point. The composer works but
needs two taps to raise the keyboard and does not feel native, bot messages
cannot be selected or copied, and emoji are monochrome with gaps. The real
application should take its composer text from a native DOM text field
overlaid on the canvas, give messages a copy route, and get the framework work
ranked [below](#framework-work).

## Environment

| Item | Value |
| --- | --- |
| Device | iPhone 12 Pro Max, iOS 26.6.2, 428 × 926 points at device pixel ratio 3 |
| Browsers | Safari 26.6.1 as a home-screen web app (its user agent reports `iPhone OS 18_7`); Chrome for iOS 155 in a tab for comparison |
| Renderer | WebGPU on every device session: adapter `apple`, selection `preferred`. No WebGPU device loss or WebGL context loss was logged |
| Stobie | `d1b35b385cdc510accbc515a8d00142a58224b48`, through `stobie new`, `stobie build-web --production` and wasm-bindgen 0.2.127 |
| Serving | One small Python server on loopback for the production package, PWA files and a log collector, published with `tailscale serve` (tailnet only, Funnel off) for a publicly trusted HTTPS certificate. Default build on port 443, CJK build on 8443 |
| Dates | 16 and 17 October 2026 |

Raw evidence: [stobie-evidence `f78d18b`, `docs/ios-pwa-spike-evidence/`](https://github.com/robchristie/stobie-evidence/tree/f78d18bda40b4c18c25ad68c14dd66afada0ea96/docs/ios-pwa-spike-evidence).
It holds the sanitised device logs, a per-session
[`summary.json`](https://github.com/robchristie/stobie-evidence/blob/f78d18bda40b4c18c25ad68c14dd66afada0ea96/docs/ios-pwa-spike-evidence/summary.json),
the automated pre-check results and screenshots, and a `stobie` CLI
transcript. The spike's source stayed local, as agreed; it is not needed to
read the evidence.

## What was built

A throwaway application made with `stobie new`, outside the repository:

- **Layout.** 20 bots (avatar colour, name, last-message preview, time). At
  640 points and wider, a 320-point sidebar beside the conversation. Below
  that, a stack: the list, then the conversation with a Back action.
- **Transcript.** 200 deterministic messages for the first bot (one-liners,
  paragraphs, long unbroken strings, emoji, Chinese, Japanese and Korean),
  user messages right-aligned and bot messages left-aligned, as egui labels in
  framed bubbles inside a `ScrollArea` with `stick_to_bottom`. A Jump to
  latest action appears when the user has scrolled up. A Long history action
  swaps in 2,000 messages.
- **Composer.** A multiline `TextEdit` observed as `MultilineTextInput`
  (`ios-spike.draft`) and a Send action, so `stobie set-text` and
  `stobie invoke ios-spike.send` drive it.
- **Simulated stream.** Send appends the message and an empty reply. `prepare`
  appends due tokens from egui's input clock, and `ui` calls
  `Frame::request_repaint_after` for the next token (80 ms; about 10.5 s per
  reply). Nothing repaints unconditionally.
- **Diagnostics.** A page script recorded the user agent, renderer
  attributes, device pixel ratio, visual viewport changes, safe-area insets,
  text and composition events on the hidden inputs, `visibilitychange`,
  `pagehide` and `pageshow`, GPU loss and startup milestones. It POSTed them
  to the server. It also wrapped `requestAnimationFrame` and hooked
  `GPUQueue.submit` and the WebGL draw calls, to tell painted frames from
  loop ticks.
- **PWA shell, written by hand.** A manifest with standalone display, icons
  and theme colour; `apple-mobile-web-app-*` tags; `viewport-fit=cover` with
  page zoom disabled; safe-area insets around a fixed shell element. The shell
  is sized from `visualViewport`, so the composer stays above the keyboard. A
  service worker serves content-addressed assets cache-first and the HTML
  network-first.
- **Fonts.** The default build has Stobie's Source Sans 3 and egui's defaults.
  The `cjk` build adds a 1.79 MB Noto Sans SC subset (GB2312 hanzi, kana,
  CJK punctuation and the corpus's own characters), installed as a
  lowest-priority fallback through `egui::Context::add_font`.

## Results

| Checklist item | Result | Evidence |
| --- | --- | --- |
| 1. Install and launch | **Works** | Both builds installed and launched standalone, including offline in Aeroplane Mode. The first standalone launch transferred the full package; later launches were served by the service worker (300–1,200 bytes, the HTML revalidation) |
| 2. Typing | **Works with caveats** | English with autocorrect and predictive text, capitals, punctuation, emoji, dictation, newlines, paste, select all and delete all worked. Each focus needs **two taps** before the keyboard appears. Caret movement and the loupe work but "don't quite feel right". Some emoji show as boxes. Pinyin was not tried |
| 3. Keyboard and layout | **Works** | The composer stayed visible above the keyboard and the layout recovered when it closed. Safe areas (47 points top, 34 bottom) and rotation were correct |
| 4. Scrolling and touch | **Works** | Momentum, scrolling while a reply streams, small targets, and no accidental zoom, selection, pull-to-refresh or edge-swipe trouble |
| 5. Reading | **Fails** for selection; text and dark mode work | Bot message text cannot be selected or copied. Text is sharp and dark mode follows the system |
| 6. Lifecycle | **Works** | App switches, lock and unlock, and up to 934 s (15.5 min) hidden: state survived and every return drew on WebGPU without loss events |
| 7. Idle cost | **Works** | 580 s in the foreground untouched: zero paints, no perceptible battery use or heat. See [Idle cost](#idle-cost) |
| 8. VoiceOver | **Works** in a quick pass | The owner reached the bot list and composer and sent a message. This is one observation, not a screen-reader qualification under the [accessibility guide](ui-guides/accessibility.md#qualification-evidence): no speech output was retained |

### Typing in detail

All 168 text-focus events and every input event landed on eframe's hidden
`<input>` text agent. iOS delivered letters, QuickType words (`The`, `most`)
and emoji as `insertText`, and backspace as `deleteContentBackward`.
Dictation arrived as growing replacements (`t`, `tes`, `test`, `testing`),
which eframe's prefix diff turns into correct edits. No composition events
were logged in any session, because Pinyin was not tried; IME composition on
iOS remains unproven.

The causes of the caveats:

- **Two taps.** On the first tap the agent takes focus but no keyboard opens.
  On the second it blurs, refocuses and the keyboard opens (the visual
  viewport shrinks from 879 to 513 points). iOS raises the keyboard only for
  a `focus()` inside the user's touch gesture. eframe focuses its agent
  afterwards, from the frame in which egui gives the field focus.
- **Caret, loupe and predictions.** The loupe and caret gestures act on the
  hidden one-pixel input, not on egui's text, so egui's caret moves at one
  remove. eframe also clears the agent's value on blur and on any input type
  other than insertion, such as backspace. iOS therefore predicts and
  autocorrects without the text before the caret.
- **Emoji.** egui's bundled emoji font is monochrome and lacks recent emoji:
  🫣 and 🪿 showed as boxes. Family, skin-tone and flag sequences render as
  separate glyphs instead of one ([screenshot](https://github.com/robchristie/stobie-evidence/blob/f78d18bda40b4c18c25ad68c14dd66afada0ea96/docs/ios-pwa-spike-evidence/precheck/chromium-webgpu/01-list.png)).
- **Selection.** Bot messages are selectable egui labels, but on touch a drag
  scrolls. egui has no long-press selection, and the ARIA layer's text is
  transparent, so nothing reaches the iOS selection and copy menu.

### Native DOM text fields

Overlaying a native text field on the canvas is the right fallback for the
composer, and it reuses an existing route. The ARIA layer already mirrors each
egui text field as a hidden `textarea`. It also turns that element's `input`
events into AccessKit `SetValue` edits that egui applies as typed input. A
composer mode that makes the layer's element visible and interactive over the
field's rectangle, with the app's font and colours, would give native focus on
the first tap, iOS caret, loupe, selection, autocorrect context, dictation and
IME composition. egui would keep the text and the actions. The costs:

- keeping its geometry in step on scroll, keyboard and rotation;
- drawing nothing beneath it in egui while it is active;
- defining focus hand-off with egui;
- styling it to match the design tokens.

For read-only message text, a long-press Copy message action is the cheap
interim. Real selection needs a DOM text layer as well.

## Payload and start times

Production packages without `wasm-opt` (`--variant none`), as `build-web
--production` writes them:

| Build | WASM raw | WASM Brotli | All assets Brotli | All assets gzip |
| --- | ---: | ---: | ---: | ---: |
| Default | 10,278,929 | 2,649,933 | 2,679,127 | 3,717,068 |
| CJK font | 12,074,198 | 3,834,604 | 3,863,794 | 5,148,803 |

The CJK subset adds 1.18 MB of Brotli. Binaryen 131 `-Oz` reduced the default
WASM to 8,110,180 bytes raw and 2,551,718 Brotli (−3.7%); it was not
start-timed.

Start times on the device, in navigation-relative milliseconds from
`window.__STOBIE_STARTUP` ("first frame" is `workspace_frame_submitted`;
"ready" is `rendering_opportunity_proxy`). They exclude iOS's launch
animation and use one network over the tailnet, so they are single samples,
not a benchmark:

| Session | Bytes transferred | WASM initialised | First frame | Ready |
| --- | ---: | ---: | ---: | ---: |
| Default, first standalone launch | 2,685,848 | 321 | 408 | 429 |
| Default, service-worker launches (2) | 300–1,200 | 130–138 | 216–235 | 242–253 |
| CJK, first standalone launch | 3,871,253 | 454 | 1,000 | 1,048 |
| CJK, service-worker launches (4) | 300–1,200 | 147–346 | 243–494 | 253–516 |
| Default, Chrome for iOS tab, first visit | 2,684,297 | 591 | 669 | 680 |
| CJK, Chrome for iOS tab, first visit | 3,869,607 | 690 | 1,218 | 1,287 |

One CJK launch hid during startup and reached its first frame after 72 s; it
is excluded. The CJK build's first frame on a cold launch took 157 ms after
the UI pass, against 18 ms for the default build, while the glyph atlas grew.
Warm CJK launches show no such gap.

## Rendering and idle cost

Over 7,756 painted frames in all device sessions, the CPU time of each frame
callback had a per-window median of 2–4 ms. The worst window's 95th percentile
was 11 ms, the longest frame 86 ms, and 10 frames exceeded 16.7 ms. This is
CPU time in JavaScript and WASM only; GPU timing is unavailable. The
transcript lays out all 200 messages every frame with no virtualisation. The
2,000-message history was not exercised on the device.

### Idle cost

In 580 s untouched in the foreground the app painted nothing, and the owner
noticed no battery use or heat. eframe's `requestAnimationFrame` loop still
ran: 34,912 ticks (60 Hz), each taking at most 4 ms of callback time and
usually under 1 ms. Hidden, the loop stops. In the pre-checks a streamed reply painted about 24
frames a second: one per token, plus layout passes.

## Automated pre-checks

`tools/precheck.mjs` in the spike ran 17 checks with Playwright 1.62.1 and an
iPhone 13 profile (touch, 390 × 664 points, device pixel ratio 3). They
covered startup, the stacked layout, the ARIA layer's rows, tapping a row and
Back, opening at the bottom, touch-drag scrolling, Jump to latest, typing,
Enter for a newline, paste, Send, a pinned 10 s stream, staying unpinned when
scrolled up mid-stream, zero idle paints and the landscape split layout.

| Run | Result |
| --- | --- |
| Chromium 151, WebGPU (SwiftShader) | 17/17 |
| Chromium 151, WebGL2 | 17/17 |
| Chromium 151, CJK build | 17/17 |
| Chromium 151, tailnet HTTPS URL, service worker controlling | 17/17 |
| WebKit 26.5 (Linux, in the Playwright container), WebGL2 fallback | 13 of 16 run: touch-drag scrolling, Jump to latest and scrolling during a stream failed; the re-pin check did not run without Jump to latest |

`stobie attach` through Chromium's DevTools endpoint showed the bot list and
the composer in `stobie observe`. `invoke ios-spike.open-bot --value 9`,
`set-text` with a two-line draft, `invoke ios-spike.send` and
`wait fact --key replies --value 1` all succeeded.

Where emulation could not stand in for the device:

- **Chromium's emulated device scale factor.** It leaves `ResizeObserver`'s
  `devicePixelContentBoxSize` unscaled, and eframe sizes the canvas from it.
  egui then sees a third of the width. The checks used
  `--force-device-scale-factor=3` with a real window instead, which gives up
  `isMobile`.
- **WebKit on Linux.** It has no `Touch` constructor, and Playwright has no
  touch drag. A mouse drag over egui's selectable labels selects text instead
  of scrolling, so the three WebKit failures say nothing about the device,
  where scrolling worked.
- **Not emulated at all.** Keyboard raising, the two-tap focus, the loupe,
  autocorrect, dictation, the visual viewport, safe areas, installation, the
  lifecycle and battery.

## Framework work

The real application would need this work, ranked by its effect on the
client. Sizes are rough: small is days, medium one to two weeks, large more.

1. **Native text input on iOS** (confirmed; medium). The first-tap keyboard
   needs eframe's text agent focused inside the touch gesture, as an eframe
   change or a host workaround. Beyond that, the composer needs the native
   field overlay [described above](#native-dom-text-fields), built on the
   ARIA layer's field mirroring. Evidence: [typing](#typing-in-detail).
2. **Message content: selection, copy and readable observation** (confirmed;
   medium). Rich text or Markdown with semantics and selection is the full
   form. A Copy message action and a DOM text layer for selection come first.
   `stobie observe` exports no page content, so the spike published a
   `last_message` fact by hand; agents need a bounded, addressable excerpt of
   message text. Evidence: checklist item 5, the CLI transcript.
3. **An async event source that wakes the host** (confirmed; medium). The
   simulation needed only `Frame::request_repaint_after`, but a socket
   delivers data outside a pass. `WebHost::with_host_mut` and
   `AppHost::submit` can apply an intent and request a repaint. `web_handle!`
   exposes neither, so the application must write its own handle. Natively it
   needs its own thread and context handle. `PassContext` also has no
   delayed repaint, so timed work in `prepare` must schedule its wake-up from
   `ui`. A channel of intents that the host drains in `prepare`, with a waker
   for each platform, would cover both.
4. **PWA output from `build-web`** (confirmed; medium). The page needs a
   manifest, icons, Apple meta tags, `viewport-fit=cover`, safe-area CSS,
   visual-viewport keyboard sizing and a service worker at a mutable path in
   scope. The production package puts every file except `index.html` under
   `assets/<content hash>/` and adds `<base href>`, so a service worker or
   manifest written into the web root would land in the hashed directory,
   outside the page's scope. The keyboard sizing was the spike's own script,
   and it is why item 3 of the checklist passed.
5. **Font fallback strategy** (confirmed; medium to large). The CJK fallback
   works but costs 1.18 MB of Brotli and 619 ms of cold-start readiness on
   Safari. Japanese kanji take Simplified Chinese glyph forms, and a
   GB2312-based subset misses rare characters. Load fallback fonts after the
   first frame, choose locale-specific faces, and decide how to show colour
   emoji and emoji sequences, which egui cannot do today.
6. **A multiline composer recipe** (confirmed; small). Growth, Send
   placement, Return versus Cmd+Enter on touch keyboards, and observation as
   `MultilineTextInput` with `set-text`. Also: `Frame::action` presents an
   `Availability::Hidden` action as a disabled button, because the button
   component expects its caller to filter hidden actions. The spike filtered
   them itself.
7. **A responsive navigation pattern** (confirmed but low; small). The
   hand-built split and stack at 640 points, Back and rotation all worked; a
   recipe would save repeating it.
8. **Variable-height virtualised lists with bottom pinning** (deferred). egui's
   `stick_to_bottom` kept 200 unvirtualised messages pinned through streaming
   and released the pin when scrolled up, within the frame times above. Revisit
   with measurements of long histories. Two traps for the recipe: an explicit
   offset of `f32::MAX` overflows egui's layout to NaN, and an animated jump
   to the bottom leaves agents observing an unpinned state mid-animation.

Smaller observations:

- `stobie build-web` has no `--features`; the CJK build edited the default
  features.
- `stobie new ios-spike` titled the app "Ios Spike".
- Browser checks with mobile emulation need a real device scale factor (see
  above).
- eframe's perpetual animation-frame loop is a measured idle cost, not a
  priority.

## Limits

- One device, one iOS version and one network. Results are single samples.
- Pinyin, the 2,000-message history on the device, and screen-reader speech
  output were not tested.
- The two-tap explanation comes from the event sequence and WebKit's rule
  that the keyboard opens only for focus inside a user gesture; no eframe
  change was tried.
- No Stobie code changed for the spike.
