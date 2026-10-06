#!/usr/bin/env node
import { writeFile } from 'node:fs/promises';
import { ApplicationClient, SocketTransport, BrowserTransport, BrowserPhysicalAdapter, NativePhysicalAdapter, describeApplicationError } from './application-client.mjs';

const help = `Usage: node tools/application-cli.mjs [connection options] COMMAND [JSON or value]
Connection: --socket PATH (or POLYORAMA_AUTOMATION_SOCKET), or --cdp URL [--url PAGE_URL]
Physical: --window X11_ID for native; --canvas SELECTOR for browser (default canvas)
Options: --timeout MS (default 15000), --output FILE, --capture PNG
Commands: hello, observe, query SELECTOR, discover SELECTOR, invoke SELECTOR,
          receipt REQUEST_ID, cancel REQUEST_ID, wait CONDITION,
          click SELECTOR, key CHORD, text VALUE, capture
Selectors and wait conditions are JSON. Selectors use exact AND fields:
id, role, name, capability, pane, domain. An ambiguous match is an error.
Invoke sends once; uncertain_completion includes the ID for receipt lookup.
Physical input reports dispatch only; observe/wait the intended result separately.
`;

async function main() {
  const flags = {};
  const values = [];
  const known = new Set(['socket', 'cdp', 'url', 'window', 'canvas', 'timeout', 'output', 'capture']);
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--help' || arg === '-h') { process.stdout.write(help); return; }
    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      if (!known.has(key) || index + 1 >= args.length) throw new Error(`Unknown or missing option ${arg}`);
      if (key in flags) throw new Error(`Repeated option ${arg}`);
      flags[key] = args[++index];
    } else values.push(arg);
  }
  const [command, input] = values;
  if (!command || values.length > 2) throw new Error(help);
  if (flags.socket && flags.cdp) throw new Error('Choose one host connection');
  const controller = new AbortController();
  process.once('SIGINT', () => controller.abort());
  const options = { signal: controller.signal, timeoutMs: flags.timeout ? Number(flags.timeout) : 15000 };
  let browser;
  let transport;
  let adapter;
  try {
    if (flags.cdp) {
      const { chromium } = await import('playwright');
      browser = await chromium.connectOverCDP(flags.cdp, { timeout: options.timeoutMs });
      const pages = browser.contexts().flatMap(context => context.pages()).filter(page => !flags.url || page.url() === flags.url);
      if (pages.length !== 1) throw new Error('Select exactly one existing browser page with --url PAGE_URL');
      transport = new BrowserTransport(pages[0]);
      adapter = new BrowserPhysicalAdapter(pages[0], { canvas: flags.canvas ?? 'canvas' });
    } else {
      transport = new SocketTransport(flags.socket);
      if (flags.window) adapter = new NativePhysicalAdapter(flags.window);
    }
    const client = new ApplicationClient(transport, options);
    const hello = await client.hello(options);
    const json = () => {
      if (!input) throw new Error(`${command} requires a JSON argument`);
      if (Buffer.byteLength(input) > 256 * 1024) throw new Error('JSON argument exceeds 256 KiB');
      return JSON.parse(input);
    };
    const physical = () => {
      if (!adapter) throw new Error('Native physical input/capture requires --window X11_ID');
      return adapter;
    };
    let result;
    switch (command) {
      case 'hello': result = { instance: client.instance, ...hello }; break;
      case 'observe': result = await client.observe(options); break;
      case 'query': result = await client.query(json(), options); break;
      case 'discover': result = await client.discover(json(), options); break;
      case 'invoke': result = await client.invoke(json(), options); break;
      case 'receipt': result = await client.receipt(input, options); break;
      case 'cancel': result = await client.cancel(input, options); break;
      case 'wait': result = await client.wait(json(), options); break;
      case 'click': result = await client.click(json(), physical(), options); break;
      case 'key': result = await client.key(input, physical(), options); break;
      case 'text': result = await client.text(input ?? '', physical(), options); break;
      case 'capture': {
        if (!flags.capture) throw new Error('capture requires --capture PNG');
        result = await client.capture(physical(), { ...options, path: flags.capture, metadataPath: flags.output,
          inputRoute: 'observation_only' });
        break;
      }
      default: throw new Error(`Unknown command ${command}\n${help}`);
    }
    const output = `${JSON.stringify(result, null, 2)}\n`;
    if (flags.output && command !== 'capture') await writeFile(flags.output, output);
    process.stdout.write(output);
    if (result.status === 'partial' || result.state === 'rejected' || result.state === 'uncertain') process.exitCode = 2;
  } finally {
    // CDP close disconnects this client; it does not shut down the external browser.
    await browser?.close();
  }
}

try { await main(); }
catch (error) {
  process.stderr.write(`${JSON.stringify({ error: describeApplicationError(error) }, null, 2)}\n`);
  process.exitCode = 2;
}
