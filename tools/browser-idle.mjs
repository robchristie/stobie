// A completed interaction may leave a scheduled egui or host frame. Establish
// quiescence before observing idle; sustained or periodic painting still fails.
//
// Under load a browser can hold back the frames an interaction scheduled
// (requestAnimationFrame waits while the GPU process is behind), so they can
// arrive after a quiet window rather than within it. One such late burst is
// settled again and recorded in `late_bursts`; a repaint after the second
// quiet window fails. Painting that recurs within about 1.4 s fails either
// way, because it repaints again in the second idle window.

/** The late bursts of frames tolerated after a quiet window. */
export const LATE_BURST_LIMIT = 1;

export async function observeWarmedIdle(readFrame, wait, { lateBurstLimit = LATE_BURST_LIMIT } = {}) {
  const read = async () => {
    const frame = await readFrame();
    if (!Number.isSafeInteger(frame) || frame < 0) {
      throw new Error(`Invalid presentation frame counter: ${frame}`);
    }
    return frame;
  };
  const initial = await read();
  const lateBursts = [];
  let settleObservedMs = 0;
  let start = initial;
  for (;;) {
    const settled = await settle(read, wait, start);
    settleObservedMs += settled.observedMs;
    await wait(700);
    const after = await read();
    if (after === settled.frame) {
      return {
        frame_before: settled.frame,
        frame_after: after,
        settle_initial_frame: initial,
        settle_final_frame: settled.frame,
        settle_observed_ms: settleObservedMs,
        idle_observed_ms: 700,
        late_bursts: lateBursts,
        deliberate_continuous_repaint: false,
      };
    }
    if (lateBursts.length >= lateBurstLimit) {
      const earlier = lateBursts.map(burst => `${burst.from} -> ${burst.to}`).join(', ');
      throw new Error(`Idle workspace repainted after settling (${settled.frame} -> ${after})${earlier ? `, after a late burst (${earlier})` : ''}`);
    }
    lateBursts.push({ from: settled.frame, to: after });
    start = after;
  }
}

// Seven unchanged samples 100 ms apart, within 3.5 s of `start`.
async function settle(read, wait, start) {
  let previous = start;
  let quietSamples = 0;
  for (let sample = 1; sample <= 35; sample++) {
    await wait(100);
    const current = await read();
    quietSamples = current === previous ? quietSamples + 1 : 0;
    previous = current;
    if (quietSamples === 7) return { frame: current, observedMs: sample * 100 };
  }
  throw new Error(`Workspace did not settle within 3500 ms (${start} -> ${previous})`);
}
