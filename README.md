# Home Speak

[![Build Status](https://travis-ci.com/dmweis/home_speak.svg?branch=master)](https://travis-ci.com/dmweis/home_speak)

Play messages submitted over MQTT or REST api into voice using google text to speech

## API token

You should be able to create API token for your google account [here](https://console.developers.google.com/apis/credentials)

## Install

``` bash
$ export GOOGLE_API_KEY=YOUR_API_TOKEN
% ./install_script
```

## Dependencies

In case build fails with alsa-sys build stepa you want to install dev dependencies for `alsa`.  
On debian the package is called `libasound2-dev`.  

```
$ sudo apt install libasound2-dev -y
```
