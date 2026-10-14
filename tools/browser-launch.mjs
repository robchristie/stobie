import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = fileURLToPath(new URL('..', import.meta.url));

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

export function hostedLinuxWebGpuLaunchOptions(platform = process.platform) {
  const headless = process.env.STOBIE_BROWSER_HEADFUL !== '1';
  const args = ['--no-sandbox', ...swiftShaderWebGpuFlags()];
  if (!headless) args.push('--ozone-platform=x11');
  else if (platform === 'linux') args.push(...headlessGpuCompositingFlags());
  return { headless, args };
}

/** Reject a Chromium `SystemInfo.getInfo` GPU report whose WebGPU canvases cannot present. */
export function assertGpuCompositing(gpu) {
  const status = gpu?.featureStatus?.gpu_compositing ?? 'unreported';
  if (status === 'enabled') return;
  const vulkan = gpu?.featureStatus?.vulkan ?? 'unreported';
  const skia = gpu?.auxAttributes?.skiaBackendType ?? 'unreported';
  throw new Error(`Chromium GPU compositing is ${status} (Vulkan ${vulkan}, Skia backend ${skia}); WebGPU canvases cannot present`);
}

// Chromium loads the UI sysroot provisioned by tools/bootstrap-linux-ui.sh
// unless the host provides those libraries (`STOBIE_USE_SYSTEM_UI_LIBS=1`) or
// no sysroot exists, as on hosts where the bootstrap does not run.
const sysrootLibraries = root => join(root, '.tools/sysroot/usr/lib');

export function uiLibrarySource(environment = process.env, root = repositoryRoot) {
  if (environment.STOBIE_USE_SYSTEM_UI_LIBS === '1') return 'system';
  return existsSync(sysrootLibraries(root)) ? 'repository-sysroot' : 'system';
}

export function uiLibraryEnvironment(environment = process.env, root = repositoryRoot) {
  if (uiLibrarySource(environment, root) === 'system') return { ...environment };
  const libraries = [sysrootLibraries(root), environment.LD_LIBRARY_PATH].filter(Boolean);
  return { ...environment, LD_LIBRARY_PATH: libraries.join(':') };
}
