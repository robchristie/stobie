import assert from 'node:assert/strict';
import test from 'node:test';
import { assertGpuCompositing, hostedLinuxWebGpuLaunchOptions } from '../browser-launch.mjs';

function launchOptions(headful) {
  const previous = process.env.STOBIE_BROWSER_HEADFUL;
  if (headful === undefined) delete process.env.STOBIE_BROWSER_HEADFUL;
  else process.env.STOBIE_BROWSER_HEADFUL = headful;
  try {
    return hostedLinuxWebGpuLaunchOptions();
  } finally {
    if (previous === undefined) delete process.env.STOBIE_BROWSER_HEADFUL;
    else process.env.STOBIE_BROWSER_HEADFUL = previous;
  }
}

test('headless launch selects ANGLE SwiftShader instead of the software-compositing default', () => {
  const options = launchOptions(undefined);
  assert.equal(options.headless, true);
  assert.ok(options.args.includes('--use-gl=angle'));
  assert.equal(options.args.filter(arg => arg.startsWith('--use-angle=')).join(), '--use-angle=swiftshader');
  assert.ok(!options.args.includes('--ozone-platform=x11'));
});

test('headful launch keeps the X11 route and the browser GL default', () => {
  const options = launchOptions('1');
  assert.equal(options.headless, false);
  assert.ok(options.args.includes('--ozone-platform=x11'));
  assert.ok(!options.args.some(arg => arg.startsWith('--use-gl=') || arg.startsWith('--use-angle=')));
});

test('GPU compositing check accepts enabled compositing', () => {
  assert.doesNotThrow(() => assertGpuCompositing({ featureStatus: { gpu_compositing: 'enabled' } }));
});

test('GPU compositing check rejects the headless software fallback', () => {
  assert.throws(() => assertGpuCompositing({
    featureStatus: { gpu_compositing: 'disabled_software' },
    auxAttributes: { glImplementationParts: '(gl=egl-angle,angle=swiftshader)' },
  }), /GPU compositing is disabled_software \(\(gl=egl-angle,angle=swiftshader\)\)/);
  assert.throws(() => assertGpuCompositing({}), /GPU compositing is unreported/);
});
