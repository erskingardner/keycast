#!/bin/bash
set -e

# Make scripts executable
chmod +x "$(dirname "$0")/generate_key.sh"
chmod +x "$0"

# Check if domain argument is provided
if [ -z "$1" ]; then
    echo "Error: Domain argument is required"
    echo "Usage: $0 <domain>"
    echo "Example: $0 keycast.example.com"
    exit 1
fi

# Strip protocol and trailing slashes from domain
DOMAIN=$(echo "$1" | sed -e 's|^[^/]*//||' -e 's|/.*$||')

echo "Using domain: $DOMAIN"

# Check if master.key exists
if [ ! -f "./master.key" ]; then
    echo "Generating master.key..."
    bash "$(dirname "$0")/generate_key.sh"
fi

# Create database directory if it doesn't exist
mkdir -p database

# Create .env from example if it doesn't exist
if [ ! -f ".env" ]; then
    echo "Creating .env file..."
    cp .env.example .env
    # Update domain in .env file
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS requires an empty string after -i
        sed -i '' "s/DOMAIN=.*/DOMAIN=$DOMAIN/" .env
    else
        # Linux version
        sed -i "s/DOMAIN=.*/DOMAIN=$DOMAIN/" .env
    fi
    echo "Updated DOMAIN in .env to: $DOMAIN"
else
    echo "Note: .env file already exists. Skipping .env creation."
    echo "If you need to update the domain, edit the .env file manually."
fi

echo "🔑 Generated master key"
echo "📁 Created database directory"
echo "⚙️  Created .env file with domain: $DOMAIN"
echo "✅ Initialization complete!"
echo ""
echo "Next steps:"
echo "1. Make sure your DNS records are set up for $DOMAIN"
echo "2. Run 'docker-compose build' to build the docker images"
echo "3. Run 'docker-compose up -d' to start the services"

