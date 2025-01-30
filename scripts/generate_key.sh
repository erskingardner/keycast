#!/bin/bash
set -e

KEY_FILE="master.key"

# Check if key already exists and --force not specified
if [ -f "$KEY_FILE" ] && [ "$1" != "--force" ]; then
    echo "Error: $KEY_FILE already exists. Use --force to overwrite."
    exit 1
elif [ -f "$KEY_FILE" ] && [ "$1" == "--force" ]; then
    echo "Warning: Overwriting existing master key!"
fi

# Generate a 32-byte (256-bit) random key using openssl and base64 encode it
# If openssl is not available, fall back to /dev/urandom
if command -v openssl >/dev/null 2>&1; then
    openssl rand 32 | base64 > "$KEY_FILE"
else
    dd if=/dev/urandom bs=32 count=1 2>/dev/null | base64 > "$KEY_FILE"
fi

# Set file permissions to 600 (read/write for owner only)
chmod 600 "$KEY_FILE"

echo "✅ Generated new master key at $KEY_FILE"
echo "🔒 Set file permissions to 600 (read/write for owner only)"
