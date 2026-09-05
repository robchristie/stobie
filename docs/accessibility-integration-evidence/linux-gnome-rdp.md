# Debian GNOME/Orca RDP accessibility qualification

Status: directly qualified for this exact human-operated Linux environment

## Evidence identity

- executable source revision:
  `fb6c9f3773a88a87a5bf5be7da8453c8a89c6c24`;
- release binary SHA-256:
  `81d9d97de48194e25bbc215a03135fe2d6c37b9ba9f7d4bab072a55c7dd1479a`;
- qualification date: 5 September 2026, Australia/Adelaide;
- guest: Debian GNU/Linux 13, kernel
  `6.12.107+deb13-cloud-amd64`, x86-64;
- desktop and transport: GNOME Shell `48.7-0+deb13u2`, GNOME Remote
  Desktop `48.1-4`, GDM remote login and RDP through a private libvirt NAT
  network and SSH tunnel;
- assistive technology: Orca `48.1-1+deb13u2`, AT-SPI2
  `2.56.2-1+deb13u1` and Speech Dispatcher `0.12.0-5`;
- virtual hardware: KVM guest with 6 vCPUs, 12 GiB RAM and a virtio VGA
  device; and
- retained VM baseline:
  `polyorama-a11y` snapshot
  `operational-accessibility-baseline-20260905` on
  `nostromo.yutani.tech`.

GNOME Remote Desktop's system service provided its documented headless
multi-user remote-login route. Pixel streaming used PipeWire, input used
libei, and GNOME Remote Desktop managed the graphical session through Mutter's
remote-desktop API. Port 3389 was reachable only on the guest's private NAT
address; the human operator connected from macOS through an SSH local-forward
and an RDP client.

## Combined qualification result

The evidence deliberately combines two complementary observations against the
same release binary:

1. A repeatable guest-native probe used Xvfb, xdotool, AT-SPI2 and Orca to
   complete the representative workflow. It recorded 131 input steps, 86
   application-state checkpoints and a 106-line live AT-SPI tree. The selected
   result, splitter adjustment, tool changes, viewport fit and Inspector state
   all reached their expected outcomes.
2. In the human-operated GNOME/RDP desktop, the repository owner enabled Orca,
   ran the same candidate and confirmed that Orca audibly read the workflow.
   This direct listening observation was reported in the qualification task on
   5 September 2026.

The headless probe's own debug transcript contained only two generated speech
lines, so it is not used as proof that the complete spoken experience was
audible. The human observation supplies that missing proof. Conversely, the
human observation is not treated as a replacement for the probe's exact input,
state and tree records. Together they establish an understandable, keyboard-
operable and audibly exposed workflow for this exact configuration.

The representative workflow covered locating the application and status;
navigating the application bar, dock tabs, splitters and pane content;
invoking Save layout and discovering unavailable actions; focusing and
changing the analytical viewport; choosing Polygon and Navigate modes;
invoking Fit view without a pointer; selecting one bounded result row; and
reading the resulting Inspector state.

## Automated artefact manifest

The repeatable run completed from `2026-09-04T23:51:18Z` to
`2026-09-04T23:54:19Z`. Its full evidence remains in the snapshotted guest at:

```text
/home/rob/polyorama-a11y-evidence/20260904T235118Z-fb6c9f3773a8
```

| Artefact | Bytes | SHA-256 |
| --- | ---: | --- |
| `environment.txt` | 430 | `b44203b946cf3fbf22f858ad6ff633bc7d5734536ce403e737d3f6fdd8efaff8` |
| `atspi-tree.txt` | 19,059 | `45ed4e1aca367931e487d80af155ad96b22efa9d15ce28584525512ac5f7c1d9` |
| `workflow-steps.log` | 4,331 | `8dbcb5c2a79911c8fd4e7022f7df73a25422f4e640cf93e83613b649fe6a8cba` |
| `workflow-state.jsonl` | 47,898 | `9591a69352cc98d5d0e8067aae036c7005e5a7bee429b81369912fd9dea7d7e8` |
| `at-snapshot.json` | 318,070 | `c64b7e5717e9a2516723b3a1a014930c2cf88983f8f1b3f976d2afc90d31130a` |
| `probe.png` | 1,039,119 | `f87a9edefb72ff66f13e982daa3ba01338ff1ae3352976ddda80f8eeeb3049c4` |
| `orca-debug.log` | 49,665 | `414606af93d6fd69dcebd005295489ae491abd1257fbae18e40f1d7f9b96f7ae` |
| `app.log` | 871 | `dbae6f00c1d0c0f340ebe390ad69af13a50613a46cdfb3d396193cf42100e10d` |

The automated renderer was eframe/wgpu OpenGL on Mesa 25.0.7 llvmpipe. That
renderer identity belongs to the supporting Xvfb probe, not to a physical-GPU
performance claim for the interactive RDP session.

## Qualification boundary

This evidence directly qualifies only the versioned Debian 13, GNOME 48,
GNOME Remote Desktop RDP and Orca 48.1 combination above. It does not establish
a general Linux claim across distributions, desktop environments, local-seat
sessions or Orca versions. Windows with NVDA or Narrator, macOS with VoiceOver,
and every browser/operating-system/assistive-technology combination remain
independently unqualified. Stock eframe 0.36.1 still discards browser AccessKit
updates, so this native result does not change the retained browser blocker.
