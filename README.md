# discord-xiv-emotes

A Discord bot that responds to commands with emote messages from FFXIV.

This README is a WIP.

The necessary environment for the bot to work is:

* `DISCORD_TOKEN` environment variable
* `DATABASE_URL` environment variable that points to a
* postgres database

The recommended way of running it by producing an executable, either through `cargo build --release`
or by downloading a release build from GitHub, and then running `docker compose up`. In this case,
you must still specify `DATABASE_URL`, either with a `.env` file or by manually defining it, and if
the executable is anywhere other than `./discord-xiv-emotes` you can specify its location with the
`EXEC` environment variable.
