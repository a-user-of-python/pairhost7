#!/usr/bin/env python3
"""Build the PairHost7 rootless .deb without dpkg-deb.

A .deb is an ar archive with exactly three members:
    debian-binary    containing "2.0\\n"
    control.tar.gz   containing ./control
    data.tar.gz      containing ./var/jb/usr/bin/pairhost7

Everything is written with root ownership and fixed timestamps so
the output is reproducible and does not depend on the build host's
tar implementation.

Usage: build_deb.py <binary> <output.deb> [version]
"""

import io
import sys
import tarfile

PACKAGE = "com.user.pairhost7"
NAME = "PairHost7"
DESCRIPTION = (
    "Wireless pairing host for iOS 27 devices. Run pairhost7 in a "
    "terminal on a jailbroken iPhone and an iOS 27 iPhone/iPad can "
    "pair with it over Wi-Fi (Settings > Privacy & Security > "
    "Developer Mode), producing an RPPairing file for StikDebug and "
    "similar tools."
)
MAINTAINER = "user <user@users.noreply.github.com>"


def _add_dir(tf, name):
    info = tarfile.TarInfo(name)
    info.type = tarfile.DIRTYPE
    info.mode = 0o755
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "wheel"
    tf.addfile(info)


def _add_file(tf, name, data, mode):
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = mode
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "wheel"
    tf.addfile(info, io.BytesIO(data))


def make_tar_gz(build):
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tf:
        build(tf)
    return buf.getvalue()


def ar_member(name, data):
    # ar member header: name(16) mtime(12) uid(6) gid(6) mode(8, octal
    # ascii) size(10) magic(2). All fields left-justified, space-padded.
    header = (
        f"{name:<16}"
        f"{'0':<12}"
        f"{'0':<6}"
        f"{'0':<6}"
        f"{'100644':<8}"
        f"{len(data):<10}"
        "`\n"
    ).encode("ascii")
    assert len(header) == 60, len(header)
    out = header + data
    if len(data) % 2 == 1:
        out += b"\n"
    return out


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    binary_path, out_path = sys.argv[1], sys.argv[2]
    version = sys.argv[3] if len(sys.argv) > 3 else "1.0"

    with open(binary_path, "rb") as f:
        binary = f.read()

    control_text = (
        f"Package: {PACKAGE}\n"
        f"Name: {NAME}\n"
        f"Version: {version}\n"
        "Architecture: iphoneos-arm64\n"
        f"Description: {DESCRIPTION}\n"
        f"Maintainer: {MAINTAINER}\n"
        f"Author: {MAINTAINER}\n"
        "Section: Utilities\n"
    )

    def build_control(tf):
        _add_file(tf, "./control", control_text.encode("utf-8"), 0o644)

    def build_data(tf):
        _add_dir(tf, "./var")
        _add_dir(tf, "./var/jb")
        _add_dir(tf, "./var/jb/usr")
        _add_dir(tf, "./var/jb/usr/bin")
        _add_file(tf, "./var/jb/usr/bin/pairhost7", binary, 0o755)

    control_tar = make_tar_gz(build_control)
    data_tar = make_tar_gz(build_data)

    deb = b"!<arch>\n"
    deb += ar_member("debian-binary", b"2.0\n")
    deb += ar_member("control.tar.gz", control_tar)
    deb += ar_member("data.tar.gz", data_tar)

    with open(out_path, "wb") as f:
        f.write(deb)
    print(f"Built {out_path} ({len(deb)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
