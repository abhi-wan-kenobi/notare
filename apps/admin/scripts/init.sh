#!/bin/bash
set -e

echo "Installing migration dependencies..."
cd /app/scripts
pnpm config set store-dir ~/.pnpm-store
pnpm install --prod
cd /app

echo "Running database migrations..."
cd /app/scripts
npx drizzle-kit migrate --config=../drizzle.config.ts
cd /app

echo "Starting server..."
exec node .output/server/index.mjs