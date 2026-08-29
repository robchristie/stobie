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

export function hostedLinuxWebGpuLaunchOptions() {
  const headless = process.env.POLYORAMA_BROWSER_HEADFUL !== '1';
  const args = ['--no-sandbox', ...swiftShaderWebGpuFlags()];
  if (!headless) args.push('--ozone-platform=x11');
  return { headless, args };
}
