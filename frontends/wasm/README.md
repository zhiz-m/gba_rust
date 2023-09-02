## prereqs
```sh
sudo apt-get install npm
cargo install wasm-pack 
cargo isntall wasm-bindgen-cli
```
(the last two may or may not be needed)

## How to install (run this if cloning a fresh copy of this repo)

```sh
npm install
```

## How to build

```sh
# Builds the project and places it into the `dist` folder.
npm run build
```

## How to run (after building)

```sh
npm install -g serve@12.0.1
serve dist
```

or, use your webserver of choice. 