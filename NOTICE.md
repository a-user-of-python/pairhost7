# Attribution and provenance

PairHost7 is built on the `idevice` Rust library by Jackson Coxson:

- Project: https://github.com/jkcoxson/idevice
- License: MIT (see `LICENSE`, which reproduces the upstream license text)
- Pinned revision used here:
  `89702099f4d7552f704f93b4526756abda319ceb`

`src/main.rs` is derived from the upstream `tools/src/pair_host.rs`
example in that repository (MIT, Jackson Coxson). Changes made here:
a persistent host identity, a retry loop instead of exiting on the
first failed pairing attempt, iOS-friendly default output paths, and
the program name/defaults. The pairing protocol implementation itself
(the `remote_pairing` module: `PairableHost`, `RpPairingFile`, the SRP
pair-setup) is entirely upstream `idevice` code, used as a library.

The remaining material in this repository (packaging script, CI
workflow, documentation) is © the repository owner, all rights
reserved.
