# Home Speak

[![codecov](https://codecov.io/gh/dmweis/home_speak/branch/main/graph/badge.svg)](https://codecov.io/gh/dmweis/home_speak)
[![Rust](https://github.com/dmweis/home_speak/workflows/Rust/badge.svg)](https://github.com/dmweis/home_speak/actions)
[![Private docs](https://github.com/dmweis/home_speak/workflows/Deploy%20Docs%20to%20GitHub%20Pages/badge.svg)](https://davidweis.dev/home_speak/home_speak/index.html)

Play messages submitted over MQTT or REST api into voice using google text to speech

## API token

### Google

You should be able to create API token for your google account [here](https://console.developers.google.com/apis/credentials)

### Azure

Generate tokens using your [Azure portal](https://portal.azure.com)

## Simple curl call

```bash
curl --data "Test string" localhost:3000/say
```

## Install

Export google api token with an env var

```bash
$ export APP_GOOGLE_API_KEY=YOUR_API_TOKEN
% ./install_script
```

or by by crating a config file `dev_settings.yaml`:

```yaml
google_api_key: "YOUR_API_TOKEN"
```

## Dependencies

In case build fails with alsa-sys build stepa you want to install dev dependencies for `alsa`.  
On debian the package is called `libasound2-dev`.

```bash
sudo apt install libasound2-dev -y
```
