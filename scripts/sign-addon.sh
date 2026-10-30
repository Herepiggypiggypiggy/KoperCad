#!/usr/bin/env bash
# Add-on signing, per user, no admin rights, no network. A thin wrapper over
# `kopercad-cli addons keygen | sign | verify`; the guide is docs/design/addons-signing.md.
#
#   scripts/sign-addon.sh keygen [KEYFILE] [--id koper-vonk-2026] [--owner KoperVonk]
#   scripts/sign-addon.sh sign   FILE.kcaddon [--key KEYFILE]       # in place
#   scripts/sign-addon.sh verify FILE.kcaddon [--keys addons/keys/kopervonk.pub]
#   scripts/sign-addon.sh secret [KEYFILE]   # puts the GitHub secret value on the clipboard
#
# The private key lives in ~/KoperCAD-keys/<id>.key (mode 600, outside the repository). `sign`
# uses, in this order: --key, the base64 text in $ADDON_SIGNING_KEY (what CI has), the single
# *.key file in ~/KoperCAD-keys. The key is never printed.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
keydir="${KOPERCAD_KEY_DIR:-$HOME/KoperCAD-keys}"

cli() {
  if [ -n "${KOPERCAD_CLI:-}" ]; then
    "$KOPERCAD_CLI" "$@"
  elif command -v kopercad-cli >/dev/null 2>&1; then
    kopercad-cli "$@"
  elif [ -x "${CARGO_TARGET_DIR:-$root/target}/release/kopercad-cli" ]; then
    "${CARGO_TARGET_DIR:-$root/target}/release/kopercad-cli" "$@"
  else
    cargo run -q --release -p novo-cli --manifest-path "$root/Cargo.toml" -- "$@"
  fi
}

only_key() {
  local found=("$keydir"/*.key)
  if [ "${#found[@]}" -eq 1 ] && [ -f "${found[0]}" ]; then
    printf '%s' "${found[0]}"
  fi
}

action="${1:-}"
[ -n "$action" ] || { sed -n 2,13p "$0" | sed 's/^# \{0,1\}//'; exit 2; }
shift

case "$action" in
  keygen)
    out=""
    if [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; then out="$1"; shift; fi
    id="koper-vonk-$(date -u +%Y)"
    args=("$@")
    for ((i = 0; i < ${#args[@]}; i++)); do
      [ "${args[i]}" = "--id" ] && id="${args[i + 1]:-$id}"
    done
    [ -n "$out" ] || out="$keydir/$id.key"
    umask 077
    mkdir -p "$(dirname "$out")"
    # --id is passed once, from the default or the user's own flag
    has_id=0
    for a in "${args[@]+"${args[@]}"}"; do [ "$a" = "--id" ] && has_id=1; done
    if [ "$has_id" = 1 ]; then
      cli addons keygen --out "$out" "${args[@]}"
    else
      cli addons keygen --out "$out" --id "$id" "${args[@]+"${args[@]}"}"
    fi
    ;;
  sign)
    file="${1:?usage: sign FILE.kcaddon [--key KEYFILE]}"
    shift
    if printf '%s\0' "$@" | grep -qz -- '^--key'; then
      cli addons sign "$file" "$@"
    elif [ -n "${ADDON_SIGNING_KEY:-}" ]; then
      cli addons sign "$file" --key-env ADDON_SIGNING_KEY "$@"
    else
      key="$(only_key)"
      [ -n "$key" ] || { echo "no key: pass --key FILE, set ADDON_SIGNING_KEY, or keep exactly one .key in $keydir" >&2; exit 2; }
      cli addons sign "$file" --key "$key" "$@"
    fi
    ;;
  verify)
    file="${1:?usage: verify FILE.kcaddon [--keys FILE]}"
    shift
    if [ $# -eq 0 ] && [ -f "$root/addons/keys/kopervonk.pub" ]; then
      cli addons verify "$file" --keys "$root/addons/keys/kopervonk.pub"
    else
      cli addons verify "$file" "$@"
    fi
    ;;
  secret)
    key="${1:-$(only_key)}"
    [ -f "$key" ] || { echo "no key file; pass one or keep exactly one .key in $keydir" >&2; exit 2; }
    value="$(base64 < "$key" | tr -d '\n')"
    if command -v pbcopy >/dev/null 2>&1; then printf '%s' "$value" | pbcopy
    elif command -v wl-copy >/dev/null 2>&1; then printf '%s' "$value" | wl-copy
    elif command -v xclip >/dev/null 2>&1; then printf '%s' "$value" | xclip -selection clipboard
    else
      umask 077
      printf '%s' "$value" > "$key.b64"
      echo "No clipboard tool found. Wrote $key.b64 (mode 600): paste its text into the GitHub secret, then delete the file."
      exit 0
    fi
    echo "The value for the GitHub secret ADDON_SIGNING_KEY is on the clipboard (not printed). Paste it in GitHub, then copy something else."
    ;;
  *)
    echo "unknown action: $action (keygen, sign, verify, secret)" >&2
    exit 2
    ;;
esac
