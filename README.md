# PairHost7

A small command-line tool for a **jailbroken iPhone** that lets it act
as the "computer" for **iOS 27 device-initiated wireless pairing** —
so an iPhone or iPad on iOS 27+ can pair with it over Wi-Fi and
produce a fresh **RPPairing file**, the file apps like **StikDebug**
need before they will connect.

It exists because every other pairing host is either a desktop
program (idevice_pair, iloader) or an app that itself needs a modern
iOS (StikDebug needs iOS 17.4+; StikPair needs iOS 27). PairHost7 is a
terminal program, so it runs on a jailbroken device on iOS 15.

## How it works

Starting with iOS 27, a device can initiate pairing to a computer
instead of the other way around:

1. PairHost7 advertises the Bonjour service
   `_remotepairing-pairable-host._tcp` on the local Wi-Fi network and
   listens on a TCP port. No USB cable and no usbmuxd are involved.
2. On the iOS 27 device, you open
   **Settings → Privacy & Security → Developer Mode**, pick the
   advertised host, and type in the 6-digit code PairHost7 prints in
   its terminal.
3. The device and PairHost7 run Apple's SRP pair-setup (implemented
   by the `idevice` Rust library, see `NOTICE.md`). When it finishes,
   PairHost7 holds the pairing file for that device.
4. You AirDrop the file to the paired device and import it into the
   app that needs it.

## Install (on the jailbroken iPhone)

Install `PairHost7.deb` with Sileo: open the `.deb` from the Files
app, share it to Sileo, and install. It is a rootless package
(Dopamine/Procursus layout) that installs one binary:

```
/var/jb/usr/bin/pairhost7
```

## Use

Both devices must be on the **same Wi-Fi network**, and the iOS 27
device must be **unlocked** with **Developer Mode** turned on.

1. Open your terminal app on the jailbroken iPhone (NewTerm, or SSH
   in) and run:

   ```
   pairhost7
   ```

   Leave it running. If networking seems restricted, run it as root
   instead: `sudo pairhost7`.

2. On the iPhone/iPad you want to pair (iOS 27+):
   **Settings → Privacy & Security → Developer Mode**, scroll to the
   pairing section, and tap **PairHost7**.

3. The terminal on the jailbroken iPhone shows a 6-digit code in a
   big banner. Type that code into the iOS 27 device.

4. When pairing completes, the terminal prints the paired device's
   name/model/UDID and writes the pairing file to:

   - `~/pairingfile.plist` (your home folder — easy to grab over SSH)
   - `/var/mobile/Media/pairingfile.plist` (easy to reach in Filza)
   - `./pairingfile.plist` (the folder you ran it from)

   It keeps advertising after a failed attempt (for example a mistyped
   code), so you can just try again on the device. Ctrl+C stops it.

   The host identity (keys + identifier) is saved in `~/.pairhost7/`
   and reused on later runs, so re-running does not invalidate a
   pairing you already made. Delete that folder if you ever want a
   completely fresh host identity.

5. Send `pairingfile.plist` to the paired device (AirDrop from Filza
   or the Files app works), then import it in **StikDebug**:
   **Settings → Pairing File → Import Pairing File** (delete the old
   pairing file in StikDebug first). Then connect LocalDevVPN and try
   StikDebug again.

   Treat the pairing file like a password: it contains a private key
   for your device's developer services. PairHost7 writes it with
   owner-only permissions; don't share it publicly.

Useful flags: `--name NAME` (what the device shows), `--out PATH`
(write the pairing file to exactly this path), `--port N`,
`--pinless`. Run `pairhost7 --help` for the list. For debugging, run
with `RUST_LOG=debug pairhost7` for a detailed protocol log.

## Build

CI (GitHub Actions, standard macOS runner) cross-compiles for
`aarch64-apple-ios` with a minimum deployment target of iOS 15.0,
signs the binary with `ldid`, and packages the rootless `.deb`
(`packaging/build_deb.py`). The `idevice` dependency is pinned in
`Cargo.toml` to revision `89702099f4d7552f704f93b4526756abda319ceb`
with only the `remote_pairing` and `rustcrypto` features enabled —
both are pure Rust, so no C crypto library has to be cross-compiled.

Local check (any platform, verifies the code compiles):

```
cargo check
cargo check --target aarch64-apple-ios
```

## Honest status

The protocol code is the same `idevice` implementation that desktop
pairing tools and on-device pairing apps are built on, and the
packaged binary is verified in CI to be a real arm64 iOS Mach-O
containing the pairable-host code. What has **not** been verified
yet, because it needs the physical devices:

- whether iOS 15 lets this terminal process advertise Bonjour/mDNS
  and receive the connection as expected,
- whether the iPad lists the host in its Developer Mode settings,
- whether StikDebug accepts the produced file (the file format is
  the same one StikDebug's own pairing flow produces).

## License

MIT for the upstream `idevice` code and the code derived from it —
see `LICENSE` and `NOTICE.md`.
