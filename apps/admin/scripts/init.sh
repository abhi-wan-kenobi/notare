#!/bin/sh

echo "Running database migrations..."
drizzle-kit migrate

echo "Starting application..."
exec pnpm start