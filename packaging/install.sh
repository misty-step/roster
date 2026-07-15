#!/bin/sh
set -eu

prefix="${HOME}/.local"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix)
            [ "$#" -ge 2 ] || { echo "install.sh: --prefix needs a path" >&2; exit 2; }
            prefix=$2
            shift 2
            ;;
        -h|--help)
            echo "usage: install.sh [--prefix PATH]"
            exit 0
            ;;
        *)
            echo "install.sh: unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

case "$prefix" in
    ""|/) echo "install.sh: refusing unsafe prefix: $prefix" >&2; exit 2 ;;
esac

package_root=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
source_root="$package_root/share/roster"
version=$(sed -n '1p' "$source_root/VERSION")
[ -n "$version" ] || { echo "install.sh: package VERSION is empty" >&2; exit 1; }
[ -x "$package_root/bin/roster" ] || { echo "install.sh: package binary is missing" >&2; exit 1; }
[ -f "$source_root/primitives/skills/skills-index.yaml" ] || {
    echo "install.sh: public library is incomplete" >&2
    exit 1
}
actual=$("$package_root/bin/roster" --version)
[ "$actual" = "roster $version" ] || {
    echo "install.sh: binary/library version mismatch: $actual vs $version" >&2
    exit 1
}

mkdir -p "$prefix/bin" "$prefix/share"
new_library="$prefix/share/.roster-new-$$"
old_library="$prefix/share/.roster-old-$$"
new_binary="$prefix/bin/.roster-new-$$"
old_binary="$prefix/bin/.roster-old-$$"

cleanup() {
    status=$?
    if [ "$status" -ne 0 ]; then
        if [ -e "$old_binary" ]; then
            rm -f "$prefix/bin/roster"
            mv "$old_binary" "$prefix/bin/roster"
        fi
        if [ -e "$old_library" ]; then
            rm -rf "$prefix/share/roster"
            mv "$old_library" "$prefix/share/roster"
        fi
    fi
    rm -rf "$new_library" "$old_library"
    rm -f "$new_binary" "$old_binary"
    exit "$status"
}
trap cleanup EXIT HUP INT TERM

mkdir "$new_library"
cp -R "$source_root/." "$new_library/"
cp "$package_root/bin/roster" "$new_binary"
chmod 755 "$new_binary"

if [ -e "$prefix/share/roster" ]; then
    mv "$prefix/share/roster" "$old_library"
fi
mv "$new_library" "$prefix/share/roster"
if [ -e "$prefix/bin/roster" ]; then
    mv "$prefix/bin/roster" "$old_binary"
fi
mv "$new_binary" "$prefix/bin/roster"

rm -rf "$old_library"
rm -f "$old_binary"
trap - EXIT HUP INT TERM
echo "installed roster $version to $prefix"
