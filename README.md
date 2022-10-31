# discord-xiv-emotes

A Discord bot that responds to commands with emote messages from FFXIV.

This README is a WIP.

The necessary environment for the bot to work is:

* `DISCORD_TOKEN` environment variable
* `DATABASE_URL` environment variable that points to a
* postgres database

Optionally, you can specify log levels with the `RUST_LOG` environment variable. The module just
the bot specifically is `discord_xiv_emotes`, so to for example enable debug logging for the bot
then `RUST_LOG` should be set to `discord_xiv_emotes=debug`.

The recommended way of running it by producing an executable, either through `cargo build --release`
or by downloading a release build from GitHub, and then running `docker compose up`. In this case,
you must still specify `DISCORD_TOKEN`, either with a `.env` file or by manually defining it, and if
the executable is anywhere other than `./discord-xiv-emotes` you can specify its location with the
`EXEC` environment variable. Alternatively, you can modify the Dockerfile and place your `.env` file
at `/dxe/.env`.

Note that when updating the executable you will need to delete the existing image, most likely named
`discord-xiv-emotes-dxe`.
