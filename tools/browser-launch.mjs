export function swiftShaderWebGpuFlags() {
  return [
    '--enable-unsafe-webgpu',
    '--use-webgpu-adapter=swiftshader',
    '--enable-dawn-features=allow_unsafe_apis',
    '--disable-dawn-features=use_dxc',
    '--enable-webgpu-developer-features',
    '--use-gpu-in-tests',
    '--enable-accelerated-2d-canvas',
    '--enable-gpu-rasterization',
    '--use-vulkan=swiftshader',
    '--disable-vulkan-fallback-to-gl-for-testing',
    '--enable-unsafe-swiftshader',
    '--enable-features=Vulkan,CDPScreenshotNewSurface',
  ];
}

// Without an explicit GL choice, chrome-headless-shell selects
// `--use-angle=swiftshader-webgl`: software compositing, in which a WebGPU
// canvas cannot allocate its swap-chain shared image. Nothing presents and the
// device is lost. Selecting ANGLE SwiftShader keeps GPU compositing on.
export function headlessGpuCompositingFlags() {
  return ['--use-gl=angle', '--use-angle=swiftshader'];
}

export function hostedLinuxWebGpuLaunchOptions() {
  const headless = process.env.STOBIE_BROWSER_HEADFUL !== '1';
  const args = ['--no-sandbox', ...swiftShaderWebGpuFlags()];
  args.push(...(headless ? headlessGpuCompositingFlags() : ['--ozone-platform=x11']));
  return { headless, args };
}

/** Reject a Chromium `SystemInfo.getInfo` GPU report whose WebGPU canvases cannot present. */
export function assertGpuCompositing(gpu) {
  const status = gpu?.featureStatus?.gpu_compositing ?? 'unreported';
  if (status === 'enabled') return;
  const implementation = gpu?.auxAttributes?.glImplementationParts ?? 'unreported GL implementation';
  throw new Error(`Chromium GPU compositing is ${status} (${implementation}); WebGPU canvases cannot present`);
}
