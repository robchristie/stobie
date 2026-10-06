// A completed interaction may leave a scheduled egui or host frame. Establish
// quiescence before observing idle; sustained or periodic painting still fails.
export async function observeWarmedIdle(readFrame, wait) {
  const read = async () => {
    const frame = await readFrame();
    if (!Number.isSafeInteger(frame) || frame < 0) {
      throw new Error(`Invalid presentation frame counter: ${frame}`);
    }
    return frame;
  };
  const initial = await read();
  let previous = initial;
  let quietSamples = 0;
  for (let sample = 1; sample <= 35; sample++) {
    await wait(100);
    const current = await read();
    quietSamples = current === previous ? quietSamples + 1 : 0;
    previous = current;
    if (quietSamples === 7) {
      await wait(700);
      const after = await read();
      if (after !== current) {
        throw new Error(`Idle workspace repainted after settling (${current} -> ${after})`);
      }
      return {
        frame_before: current,
        frame_after: after,
        settle_initial_frame: initial,
        settle_final_frame: current,
        settle_observed_ms: sample * 100,
        idle_observed_ms: 700,
        deliberate_continuous_repaint: false,
      };
    }
  }
  throw new Error(`Workspace did not settle within 3500 ms (${initial} -> ${previous})`);
}
