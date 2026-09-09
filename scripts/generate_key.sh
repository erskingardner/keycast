#!/bin/bash
set -euo pipefail
umask 077

# init.sh sets this when state lives outside the checkout.
KEY_FILE="${KEYCAST_ROOT_KEY_PATH:-master.key}"

# Never offer an overwrite shortcut: replacing this file makes every stored key undecryptable.
if [ -f "$KEY_FILE" ]; then
    echo "Error: $KEY_FILE already exists; refusing to overwrite it."
    exit 1
fi

key_directory="$(cd "$(dirname "$KEY_FILE")" && pwd)"
temporary_key=$(mktemp "$key_directory/.keycast-root.XXXXXX")
trap 'rm -f "$temporary_key"' EXIT

# Generate a 32-byte (256-bit) random key using openssl and base64 encode it
# If openssl is not available, fall back to /dev/urandom
if command -v openssl >/dev/null 2>&1; then
    openssl rand 32 | base64 > "$temporary_key"
else
    dd if=/dev/urandom bs=32 count=1 2>/dev/null | base64 > "$temporary_key"
fi

# Publish only a complete credential, without overwriting any existing path.
ln "$temporary_key" "$KEY_FILE"

# Set file permissions to 600 (read/write for owner only)
chmod 600 "$KEY_FILE"

echo "✅ Generated new master key at $KEY_FILE"
echo "🔒 Set file permissions to 600 (read/write for owner only)"
