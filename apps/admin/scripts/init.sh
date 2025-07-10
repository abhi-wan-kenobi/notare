#!/bin/sh

echo "Running database migrations..."
if [ -f "./node_modules/.bin/drizzle-kit" ]; then
    ./node_modules/.bin/drizzle-kit migrate
elif command -v pnpm > /dev/null; then
    pnpm exec drizzle-kit migrate
else
    npx drizzle-kit migrate
fi

echo "Starting application..."
if command -v pnpm > /dev/null; then
    exec pnpm start
else
    exec npm start
fi