# Web Scraper Subs Rust

### Setting up a web driver
- download the latest verison of the [gecko driver](https://github.com/mozilla/geckodriver/releases)
```
wget https://github.com/mozilla/geckodriver/releases/download/v0.33.0/geckodriver-v0.33.0-linux64.tar.gz
```
- extract the file
```
tar -xvzf geckodriver*
```
- make it executable
```
chmod +x geckodriver
```
- move it to make it globally executable
```
sudo mv geckodriver /usr/local/bin/
```

### Transmission RPC
- Ping Transmission
```
ping <transmission_address>
```
- Check Transmission RPC API with curl, get session id
```
curl -i http://<transmission_address>:9091/transmission/rpc
```
- Add a Torrent with a Magnet Link
```
curl -i -H "X-Transmission-Session-Id: YOUR_SESSION_ID" \
     -H "Content-Type: application/json" \
     -X POST -d '{"method":"torrent-add","arguments":{"filename":"YOUR_MAGNET_LINK"}}' \
     http://<transmission_address>:9091/transmission/rpc
```

