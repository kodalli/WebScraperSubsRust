# Web Scraper Subs Rust

### Scraper Logic

## Setting up a web driver
- Download the latest verison of the [gecko driver](https://github.com/mozilla/geckodriver/releases)
```shell
wget https://github.com/mozilla/geckodriver/releases/download/v0.33.0/geckodriver-v0.33.0-linux64.tar.gz
```
- Extract the file
```shell
tar -xvzf geckodriver*
```
- Make it executable
```shell
chmod +x geckodriver
```
- Move it to make it globally executable
```shell
sudo mv geckodriver /usr/local/bin/
```

## Transmission RPC
- Ping Transmission
```shell
ping <transmission_address>
```
- Check Transmission RPC API with curl, get session id
```shell
curl -i http://<transmission_address>:9091/transmission/rpc
```
- Add a Torrent with a Magnet Link
```shell
curl -i -H "X-Transmission-Session-Id: YOUR_SESSION_ID" \
     -H "Content-Type: application/json" \
     -X POST -d '{"method":"torrent-add","arguments":{"filename":"YOUR_MAGNET_LINK"}}' \
     http://<transmission_address>:9091/transmission/rpc
```

## Local Development

### Install node modules
```shell
pnpm i
```

### Install cargo-make
```shell
cargo install cargo-make
```

### Building tailwind css
```shell
cargo make styles
```
> This runs a watch task that will rebuild the css when the `styles/tailwind.css` file is changed.

### Running the Development Server
```shell
cargo make run
```
> This wil run a daemon that will rebuild the project when the source code is changed.

## Subsplease

### Xpaths

- Source image
```shell
"//img[contains(@class, 'img-responsive')][contains(@class, 'img-center')]"
```
- Episode title
```
"//label[contains(@class, 'episode-title')]"
```



