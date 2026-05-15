#!/bin/bash
set -e

# Make scripts executable
chmod +x "$(dirname "$0")/generate_key.sh"
chmod +x "$0"

# Function to print usage
print_usage() {
    echo "Usage: $0 --domain <domain> --allowed-pubkeys <pubkeys> [--uid <uid>] [--gid <gid>]"
    echo "Example:"
    echo "  $0 --domain keycast.example.com --allowed-pubkeys \"hexpubkey1,hexpubkey2\""
    echo "Allowed pubkeys are required for Docker deployments."
    exit 1
}

KEYCAST_UID="${KEYCAST_UID:-$(id -u)}"
KEYCAST_GID="${KEYCAST_GID:-$(id -g)}"

# Parse named arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --domain) DOMAIN="$2"; shift ;;
        --allowed-pubkeys) ALLOWED_PUBKEYS="$2"; shift ;;
        --uid) KEYCAST_UID="$2"; shift ;;
        --gid) KEYCAST_GID="$2"; shift ;;
        *) echo "Unknown parameter: $1"; print_usage ;;
    esac
    shift
done

# Check if domain is provided
if [ -z "$DOMAIN" ]; then
    echo "Error: --domain argument is required"
    print_usage
fi

if [ -z "$ALLOWED_PUBKEYS" ]; then
    echo "Error: --allowed-pubkeys argument is required"
    print_usage
fi

# Strip protocol and trailing slashes from domain
DOMAIN=$(echo "$DOMAIN" | sed -e 's|^[^/]*//||' -e 's|/.*$||')

echo "Using domain: $DOMAIN"
echo "Using allowed pubkeys: $ALLOWED_PUBKEYS"
echo "Using runtime uid/gid: $KEYCAST_UID:$KEYCAST_GID"

# Check if master.key exists
GENERATED_MASTER_KEY=false
if [ ! -f "./master.key" ]; then
    echo "Generating master.key..."
    bash "$(dirname "$0")/generate_key.sh"
    GENERATED_MASTER_KEY=true
fi

# Create database directory if it doesn't exist
mkdir -p database
chmod 700 database
chmod 600 master.key

if chown -R "$KEYCAST_UID:$KEYCAST_GID" database 2>/dev/null \
    && chown "$KEYCAST_UID:$KEYCAST_GID" master.key 2>/dev/null; then
    echo "Set runtime ownership on database/ and master.key"
else
    echo "Note: could not set runtime ownership on database/ and master.key."
    echo "If containers cannot read master.key or write the database, run:"
    echo "  sudo chown -R $KEYCAST_UID:$KEYCAST_GID database master.key"
fi

# Create .env from example if it doesn't exist
if [ ! -f ".env" ]; then
    echo "Creating .env file..."
    cp .env.example .env

    # Update domain in .env file
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS requires an empty string after -i
        sed -i '' "s/DOMAIN=.*/DOMAIN=$DOMAIN/" .env
        # Update allowed pubkeys (escape for sed)
        ESCAPED_PUBKEYS=$(echo "${ALLOWED_PUBKEYS:-}" | sed 's/[\/&]/\\&/g')
        sed -i '' "s/ALLOWED_PUBKEYS=.*/ALLOWED_PUBKEYS=$ESCAPED_PUBKEYS/" .env
        sed -i '' "s/KEYCAST_UID=.*/KEYCAST_UID=$KEYCAST_UID/" .env
        sed -i '' "s/KEYCAST_GID=.*/KEYCAST_GID=$KEYCAST_GID/" .env
    else
        # Linux version
        sed -i "s/DOMAIN=.*/DOMAIN=$DOMAIN/" .env
        # Update allowed pubkeys (escape for sed)
        ESCAPED_PUBKEYS=$(echo "${ALLOWED_PUBKEYS:-}" | sed 's/[\/&]/\\&/g')
        sed -i "s/ALLOWED_PUBKEYS=.*/ALLOWED_PUBKEYS=$ESCAPED_PUBKEYS/" .env
        sed -i "s/KEYCAST_UID=.*/KEYCAST_UID=$KEYCAST_UID/" .env
        sed -i "s/KEYCAST_GID=.*/KEYCAST_GID=$KEYCAST_GID/" .env
    fi
    echo "Updated DOMAIN in .env to: $DOMAIN"
    echo "Updated ALLOWED_PUBKEYS in .env to: $ALLOWED_PUBKEYS"
    echo "Updated KEYCAST_UID/KEYCAST_GID in .env to: $KEYCAST_UID:$KEYCAST_GID"
else
    echo "Note: .env file already exists. Skipping .env creation."
    echo "If you need to update the values, edit the .env file manually."
fi

echo "✅ Initialization complete!"
if [ "$GENERATED_MASTER_KEY" = true ]; then
    echo "🔑 Generated master key"
else
    echo "🔑 Reused existing master key"
fi
echo "📁 Created database directory"
echo "⚙️  Created .env file with:"
echo "   - Domain: $DOMAIN"
echo "   - Allowed pubkeys: $ALLOWED_PUBKEYS"
echo "   - Runtime uid/gid: $KEYCAST_UID:$KEYCAST_GID"
echo ""
echo "Next steps:"
echo "1. Make sure your DNS records are set up for $DOMAIN"
echo "2. Run 'docker-compose build' to build the docker images"
echo "3. Run 'docker-compose up -d' to start the services"
