// Pressure is an explicit consumer workload; the inherited path stays default.
export function pressureWorkload(args) {
  if (!args.length) return 'image-gallery';
  if (args.length !== 2 || args[0] !== '--pressure-workload'
      || !['image-gallery', 'real-scene-pan-sweep'].includes(args[1])) {
    throw new Error('expected --pressure-workload image-gallery|real-scene-pan-sweep');
  }
  return args[1];
}
export function panSweep(catalogue) {
  const candidates = catalogue.map((manifest, index) => ({index, profile: manifest.identity.profile}))
    .filter(({profile: p}) => p.components === 1 && Number.isInteger(p.width)
      && Number.isInteger(p.height) && p.width >= 512 && p.height >= 512);
  if (!candidates.length) throw new Error('PAN sweep requires a single-component parent at least 512 by 512');
  candidates.sort((a,b) => b.profile.width*b.profile.height-a.profile.width*a.profile.height);
  const {index: panIndex, profile} = candidates[0];
  const factor = 512/Math.max(profile.width,profile.height);
  return {panIndex, profile, factor,
    viewWidth: Math.max(32,profile.width*factor), viewHeight: Math.max(32,profile.height*factor)};
}
