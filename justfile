# List the recipes.
default:
    @just --list

# Install the web editor's npm dependencies.
[working-directory: 'web']
web-install:
    npm install

# Build the wasm player and the web editor into web/dist.
[working-directory: 'web']
web-build: web-install
    npm run build

# Start the web editor's dev server (rebuilds the wasm first).
[working-directory: 'web']
web-dev: web-install
    npm run dev

# Build the web editor and serve the production build.
[working-directory: 'web']
web-preview: web-build
    npm run preview
